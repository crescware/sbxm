//! `sbxm destroy`。
//!
//! 対象Sandboxとsbxmの管理情報を破棄し、案件を`unmanaged`へ戻す。host cloneと
//! 利用者が管理する成果物は保持する。

use std::path::Path;
use std::time::Instant;

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::metadata::{CreationMode, ProjectMetadata};
use crate::msg;
use crate::paths::{self, ExclusiveLock, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use super::daemon;
use super::inventory::{self, Poll, ProjectState};
use super::protection::{self, Unmanaged, WorktreeReport};
use super::select::{self, ProjectPrompt};

/// 削除対象・保持対象の1件。
#[derive(Debug, Clone)]
pub enum Target {
    /// hostのpath。翻訳しない。
    Path(String),
    /// pathで示せない対象。選択した言語で説明する。
    Described(Msg),
}

/// 削除前に見せる内容。
#[derive(Debug, Clone)]
pub struct DestroyPlan {
    pub project: String,
    pub sandbox: String,
    pub state: ProjectState,
    /// データ保護検査とactive session検査を省略するか。
    pub force: bool,
    /// 通常modeで観測したworktree。force modeでは空。
    pub worktrees: Vec<WorktreeReport>,
    pub removes: Vec<Target>,
    pub keeps: Vec<Target>,
    /// 再登録に使うcommand。
    pub re_register: String,
}

/// lockを保持したまま確認を挟むための状態。
#[derive(Debug)]
pub struct Prepared {
    pub plan: DestroyPlan,
    paths: ProjectPaths,
    name: SandboxName,
    state: ProjectState,
    force: bool,
    _lock: ExclusiveLock,
}

/// 削除の結果。
#[derive(Debug, Clone)]
pub struct DestroyOutcome {
    pub project: String,
    pub re_register: String,
    pub warnings: Vec<Msg>,
}

/// 対象を特定し、削除して良い状態であることを確かめる。
pub fn prepare(
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    force: bool,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
) -> Result<Prepared> {
    // 対象が決まる前にhostの状態へ触れない。
    let candidate = select::one(config, requested, prompt)?;
    let paths = candidate.paths.clone();

    let lock = paths::acquire_exclusive_lock(
        &paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )?;

    // lockを取る前に読んだmetadataは古くなり得る。判定はlock後の内容だけで行う。
    let metadata = candidate.reload()?;
    let name = metadata.sandbox_name();
    let entries = daemon::list(host)?;
    let state = inventory::state_of(&entries, &metadata, workspace_root)?;

    let worktrees = if force || state == ProjectState::NotCreated {
        Vec::new()
    } else {
        if state == ProjectState::Stopped {
            // 停止中のSandboxは内部を観測できないため、通常modeでは削除しない。
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::SandboxNotRunning,
                    msg!(
                        "error-sandbox-not-running",
                        sandbox = name,
                        observed = "stopped"
                    ),
                )
                .remediation(msg!(
                    "remediation-destroy-force",
                    command = format!("sbxm destroy --force {}", metadata.display_id())
                )),
            ));
        }
        let layout = SandboxLayout::new(&metadata.canonical_id);
        protection::inspect(host, name.as_str(), &layout, &metadata, Unmanaged::Allowed)?.worktrees
    };

    let plan = DestroyPlan {
        project: metadata.display_id(),
        sandbox: name.as_str().to_string(),
        state,
        force,
        worktrees,
        removes: removes(&paths, &name, state),
        keeps: keeps(&paths),
        re_register: re_register(&metadata),
    };

    Ok(Prepared {
        plan,
        paths,
        name,
        state,
        force,
        _lock: lock,
    })
}

/// Sandboxと管理情報を削除する。
///
/// metadataの削除を管理解除のcommit pointとし、最後にlock fileを削除する。
pub fn execute(
    host: &dyn HostEnvironment,
    prepared: &Prepared,
    poll: Poll,
) -> Result<DestroyOutcome> {
    if prepared.state != ProjectState::NotCreated {
        if !prepared.force {
            // 削除の直前に、sessionが接続していないことを確かめ直す。
            require_no_active_session(host, prepared.name.as_str())?;
        }
        remove_sandbox(host, &prepared.name, prepared.force, poll)?;
    }
    require_absent(host, &prepared.name)?;

    // 削除もほかのmutationと同じ規則で行う。symlinkの先を消さない。
    let cache = prepared.paths.cache_dir();
    if paths::is_symlink(&cache) {
        return Err(PathScope::ProjectPath.symlink_error(&cache));
    }
    if cache.exists() {
        std::fs::remove_dir_all(&cache).map_err(|error| cleanup_failed(&cache, &error))?;
    }

    let metadata_file = prepared.paths.metadata_file();
    if paths::is_symlink(&metadata_file) {
        return Err(PathScope::ProjectPath.symlink_error(&metadata_file));
    }
    if metadata_file.exists() {
        // ここが管理解除のcommit pointである。
        std::fs::remove_file(&metadata_file)
            .map_err(|error| cleanup_failed(&metadata_file, &error))?;
    }

    // 管理解除後は、lock fileの残存だけを警告として扱う。
    let mut warnings = Vec::new();
    let lock_file = prepared.paths.lock_file();
    if lock_file.exists()
        && let Err(error) = std::fs::remove_file(&lock_file)
    {
        warnings.push(msg!(
            "warning-lock-file-left-behind",
            path = paths::display(&lock_file),
            detail = error
        ));
    }

    Ok(DestroyOutcome {
        project: prepared.plan.project.clone(),
        re_register: prepared.plan.re_register.clone(),
        warnings,
    })
}

fn remove_sandbox(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    force: bool,
    poll: Poll,
) -> Result<()> {
    let mut args = vec!["rm"];
    if force {
        args.push("--force");
    }
    args.push(name.as_str());
    let spec = CommandSpec::passthrough("sbx", &args)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;

    let deadline = Instant::now() + poll.limit;
    loop {
        if !daemon::list(host)?
            .iter()
            .any(|entry| entry.name == name.as_str())
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::new(
                ErrorId::SandboxStillPresent,
                msg!("error-sandbox-still-present", sandbox = name),
            ));
        }
        std::thread::sleep(poll.interval);
    }
}

/// Sandboxが存在しないことを1回確認する。
fn require_absent(host: &dyn HostEnvironment, name: &SandboxName) -> Result<()> {
    if daemon::list(host)?
        .iter()
        .any(|entry| entry.name == name.as_str())
    {
        return Err(Error::new(
            ErrorId::SandboxStillPresent,
            msg!("error-sandbox-still-present", sandbox = name),
        ));
    }
    Ok(())
}

fn require_no_active_session(host: &dyn HostEnvironment, name: &str) -> Result<()> {
    let entries = daemon::list(host)?;
    let Some(entry) = entries.iter().find(|entry| entry.name == name) else {
        return Ok(());
    };
    match entry.active_sessions {
        Some(0) => Ok(()),
        Some(count) => Err(Error::single(
            Diagnostic::new(
                ErrorId::DaemonSessionActive,
                msg!(
                    "error-daemon-session-active",
                    sandboxes = format!("{name} ({count})")
                ),
            )
            .remediation(msg!("remediation-daemon-session-active")),
        )),
        None => Err(Error::single(
            Diagnostic::new(
                ErrorId::DaemonSessionUnobservable,
                msg!("error-daemon-session-unobservable", sandbox = name),
            )
            .remediation(msg!("remediation-daemon-session-unobservable")),
        )),
    }
}

/// 削除対象。
fn removes(paths: &ProjectPaths, name: &SandboxName, state: ProjectState) -> Vec<Target> {
    let mut removes = Vec::new();
    if state != ProjectState::NotCreated {
        removes.push(Target::Described(msg!(
            "destroy-target-sandbox",
            sandbox = name
        )));
    }
    removes.push(Target::Path(paths::display(&paths.metadata_file())));
    removes.push(Target::Path(paths::display(&paths.lock_file())));
    removes.push(Target::Path(paths::display(&paths.cache_dir())));
    removes
}

/// 保持対象。
fn keeps(paths: &ProjectPaths) -> Vec<Target> {
    vec![
        Target::Path(paths::display(&paths.host_clone())),
        Target::Path(paths::display(&paths.dockerfile())),
        Target::Described(msg!("destroy-target-host-images")),
        Target::Described(msg!("destroy-target-secrets")),
    ]
}

/// 元の目標構成を、新規登録として再現するcommand。
fn re_register(metadata: &ProjectMetadata) -> String {
    let provisioning = &metadata.provisioning;
    match provisioning.mode {
        CreationMode::Attached => format!(
            "sbxm add {} --worktrees {}",
            metadata.display_id(),
            provisioning.requested_worktrees
        ),
        CreationMode::Detached => format!(
            "sbxm add {} --worktrees {} --detach {}",
            metadata.display_id(),
            provisioning.requested_worktrees,
            provisioning.start_ref.clone().unwrap_or_default()
        ),
    }
}

/// 管理情報の削除に失敗した。残ったpathを示す。
fn cleanup_failed(path: &Path, error: &std::io::Error) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::CleanupFailed,
            msg!(
                "error-cleanup-failed",
                path = paths::display(path),
                detail = error
            ),
        )
        .remediation(msg!(
            "remediation-cleanup-failed",
            path = paths::display(path)
        )),
    )
}

/// 削除確認。TTYの通常modeだけがSandbox名の完全入力を求める。
pub trait ConfirmPrompt {
    fn confirm_sandbox_name(&mut self, expected: &str) -> Result<bool>;
}

/// dialoguerを使う対話実装。
pub struct TerminalConfirmPrompt;

impl ConfirmPrompt for TerminalConfirmPrompt {
    fn confirm_sandbox_name(&mut self, expected: &str) -> Result<bool> {
        let catalog = crate::i18n::Catalog::new(crate::i18n::Locale::SOURCE);
        let heading = catalog
            .text("destroy-confirm-prompt")
            .unwrap_or_else(|failure| failure.to_string());
        let typed = dialoguer::Input::<String>::new()
            .with_prompt(heading)
            .allow_empty(true)
            .interact_text()
            .map_err(|error| match error {
                dialoguer::Error::IO(io) if io.kind() == std::io::ErrorKind::Interrupted => {
                    Error::Canceled
                }
                _ => Error::new(
                    ErrorId::ProjectArgumentRequired,
                    msg!("error-project-argument-required", command = "sbxm destroy"),
                ),
            })?;
        // yes/noでは削除しない。完全一致だけを続行の合図とする。
        Ok(typed.trim() == expected)
    }
}

/// 入力が一致しない場合のerror。
pub fn confirmation_mismatch(expected: &str) -> Error {
    Error::new(
        ErrorId::DestroyNotConfirmed,
        msg!("error-destroy-not-confirmed", sandbox = expected),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata;
    use crate::workflow::inventory::tests::{FakeSbx, fixture};
    use crate::workflow::protection::tests::clean_host;
    use crate::workflow::select::tests::ScriptedPrompt;
    use std::time::Duration;

    fn poll() -> Poll {
        Poll {
            interval: Duration::from_millis(1),
            limit: Duration::from_millis(20),
        }
    }

    fn project_id(value: &str) -> ProjectId {
        ProjectId::parse(value).expect("valid project id")
    }

    fn path_of(target: &Target) -> Option<&str> {
        match target {
            Target::Path(path) => Some(path.as_str()),
            Target::Described(_) => None,
        }
    }

    #[test]
    fn a_clean_running_project_is_planned_then_removed() {
        let fixture = fixture();
        let project = fixture.register("Example-Org/Example-Repo");
        std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
        std::fs::create_dir_all(project.paths.cache_dir()).unwrap();
        // 削除後の一覧では対象が消えている。
        let host = clean_host(&fixture, &project);
        host.listing.borrow_mut().insert(0, "[]".to_string());

        let prepared = prepare(
            &fixture.config,
            Some(&project_id("Example-Org/Example-Repo")),
            false,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
        )
        .expect("prepare");

        assert_eq!(prepared.plan.project, "Example-Org/Example-Repo");
        assert_eq!(prepared.plan.worktrees.len(), 1);
        assert!(
            prepared
                .plan
                .removes
                .iter()
                .any(|target| path_of(target).is_some_and(|path| path.contains("project.toml")))
        );
        assert!(
            prepared
                .plan
                .keeps
                .iter()
                .any(|target| path_of(target).is_some_and(|path| path.contains("Dockerfile")))
        );
        assert_eq!(
            prepared.plan.re_register,
            "sbxm add Example-Org/Example-Repo --worktrees 1"
        );

        let outcome = execute(&host, &prepared, poll()).expect("destroy");
        assert!(outcome.warnings.is_empty());
        assert!(host.ran(&format!("rm {}", project.sandbox)));
        assert!(
            !project.paths.metadata_file().exists(),
            "the project is unmanaged now"
        );
        assert!(!project.paths.cache_dir().exists());
        assert!(!project.paths.lock_file().exists());
        assert!(
            project.paths.dockerfile().exists(),
            "the Dockerfile the user edits is kept"
        );
    }

    #[test]
    fn a_stopped_project_is_refused_in_the_normal_mode_and_removed_with_force() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let stopped = format!("[{}]", fixture.entry(&project, "stopped"));

        let host = FakeSbx::listing(&stopped);
        let error = prepare(
            &fixture.config,
            Some(&project_id("example-org/example-repo")),
            false,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
        )
        .expect_err("a stopped sandbox cannot be inspected");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));

        let host = FakeSbx::listings(&[&stopped, "[]"]);
        let prepared = prepare(
            &fixture.config,
            Some(&project_id("example-org/example-repo")),
            true,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
        )
        .expect("force skips the checks");
        assert!(prepared.plan.force);
        assert!(prepared.plan.worktrees.is_empty());

        execute(&host, &prepared, poll()).expect("destroy");
        assert!(host.ran("rm --force"));
        assert!(!project.paths.metadata_file().exists());
    }

    #[test]
    fn unsaved_work_stops_the_normal_mode_before_anything_is_deleted() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let name = project.sandbox.as_str();
        let managed = format!("{}/example-repo.tree-0", layout.bare_root());
        let host = clean_host(&fixture, &project).answering(
            &format!(
                "exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"
            ),
            0,
            "1 .M N... 100644 100644 100644 abc abc file.txt\0",
        );

        let error = prepare(
            &fixture.config,
            Some(&project_id("example-org/example-repo")),
            false,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
        )
        .expect_err("work that only exists here is not deleted");
        assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
        assert!(!host.ran("rm "), "nothing is removed");
        assert!(project.paths.metadata_file().exists());
    }

    #[test]
    fn a_project_without_a_sandbox_only_loses_its_management_data() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let host = FakeSbx::listing("[]");

        let prepared = prepare(
            &fixture.config,
            Some(&project_id("example-org/example-repo")),
            false,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
        )
        .expect("prepare");
        assert_eq!(prepared.plan.state, ProjectState::NotCreated);

        execute(&host, &prepared, poll()).expect("destroy");
        assert!(!host.ran("rm "), "there is no sandbox to remove");
        assert!(!project.paths.metadata_file().exists());
    }

    #[test]
    fn the_re_registration_command_repeats_the_target_configuration() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let mut metadata = project.metadata.clone();
        metadata.provisioning.mode = CreationMode::Detached;
        metadata.provisioning.start_ref = Some("develop".into());
        metadata.provisioning.requested_worktrees = 3;
        metadata.managed_worktrees.clear();
        metadata::update(&project.paths, &metadata).unwrap();

        assert_eq!(
            re_register(&metadata),
            "sbxm add example-org/example-repo --worktrees 3 --detach develop"
        );
    }

    #[test]
    fn a_cache_that_is_a_symlink_is_not_followed_and_the_project_stays_managed() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let elsewhere = fixture.workspace_root.parent().unwrap().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("keep.txt"), "not ours\n").unwrap();
        std::os::unix::fs::symlink(&elsewhere, project.paths.cache_dir()).unwrap();

        let host = clean_host(&fixture, &project);
        host.listing.borrow_mut().insert(0, "[]".to_string());
        let prepared = prepare(
            &fixture.config,
            Some(&project_id("example-org/example-repo")),
            false,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
        )
        .expect("prepare");

        let error = execute(&host, &prepared, poll()).expect_err("a symlinked cache is refused");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
        assert!(
            elsewhere.join("keep.txt").exists(),
            "what the link pointed at is untouched"
        );
        assert!(
            project.paths.metadata_file().exists(),
            "the project stays managed so the state can be settled"
        );
    }

    #[test]
    fn a_sandbox_that_survives_its_removal_keeps_the_management_data() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        // 削除後の一覧にも対象が残り続ける。
        let host = clean_host(&fixture, &project);

        let prepared = prepare(
            &fixture.config,
            Some(&project_id("example-org/example-repo")),
            false,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
        )
        .expect("prepare");

        let error = execute(&host, &prepared, poll()).expect_err("the sandbox is still there");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxStillPresent));
        assert!(
            project.paths.metadata_file().exists(),
            "the project stays managed so destroy can be run again"
        );
    }
}

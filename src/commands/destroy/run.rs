//! `sbxm destroy`。
//!
//! 対象Sandboxとsbxmの管理情報を破棄し、案件を`unmanaged`へ戻す。host cloneと
//! 利用者が管理する成果物は保持する。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::ConfigLocation;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::metadata::{self, CreationMode, ProjectMetadata};
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};
use crate::project::{CanonicalProjectId, ProjectId, SandboxLayout, SandboxName};
use crate::registry::{RegistryEntry, RegistryGuard};
use crate::repository::RepositoryIdentity;

use crate::support::daemon;
use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::protection::{self, Unmanaged, WorktreeReport};
use crate::support::secret;
use crate::support::select::{self, ProjectPrompt};
use crate::ui::{ProgressSink, PromptUi, Remediation, Warning};

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
    locked: select::Locked,
}

impl Prepared {
    /// 管理解除後にregistryから外す案件。
    pub fn unregistration(&self) -> Unregistration {
        Unregistration {
            paths: self.paths.clone(),
            repository: self.locked.metadata.repository.clone(),
        }
    }
}

/// この実行が管理を解いた案件。registry entryを外す条件の観測に使う。
///
/// project lockを手放したあとで判定するため、canonical project IDだけでなく、
/// destroyがcommitした状態を確かめるのに要る情報をまとめて持つ。
#[derive(Debug, Clone)]
pub struct Unregistration {
    paths: ProjectPaths,
    repository: RepositoryIdentity,
}

impl Unregistration {
    fn canonical_id(&self) -> &CanonicalProjectId {
        self.repository.canonical_id()
    }
}

/// 削除の結果。
#[derive(Debug, Clone)]
pub struct DestroyOutcome {
    pub project: String,
    pub re_register: String,
    pub warnings: Vec<Warning>,
}

/// 対象を特定し、削除して良い状態であることを確かめる。
pub fn prepare(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    force: bool,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
) -> Result<Prepared> {
    // 対象が決まる前にhostの状態へ触れない。
    let locked =
        select::one(location, requested, &msg!("select-destroy-heading"), prompt)?.lock()?;
    let paths = locked.paths.clone();

    let metadata = &locked.metadata;
    let name = metadata.sandbox_name();
    let entries = daemon::list(host)?;
    let state = inventory::state_of(&entries, metadata, workspace_root)?;

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
                .remediation(
                    Remediation::text(msg!("remediation-destroy-force"))
                        .try_run(format!("sbxm destroy --force {}", metadata.display_id())),
                ),
            ));
        }
        let layout = SandboxLayout::new(metadata.canonical_id());
        protection::inspect(host, name.as_str(), &layout, metadata, Unmanaged::Allowed)?.worktrees
    };

    let plan = DestroyPlan {
        project: metadata.display_id(),
        sandbox: name.as_str().to_string(),
        state,
        force,
        worktrees,
        removes: removes(&paths, &name, state),
        keeps: keeps(&paths),
        re_register: re_register(&paths, metadata)?,
    };

    Ok(Prepared {
        plan,
        paths,
        name,
        state,
        force,
        locked,
    })
}

/// Sandboxと管理情報を削除する。
///
/// metadataの削除を管理解除のcommit pointとし、最後にlock fileを削除する。
pub fn execute(
    host: &dyn HostEnvironment,
    prepared: &Prepared,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<DestroyOutcome> {
    if prepared.state == ProjectState::NotCreated {
        // 削除commandを実行しない場合だけ、一覧で不在を1回確かめる。
        require_absent(host, &prepared.name)?;
    } else {
        // 削除は、一覧から消えたことを確かめるまで完了しない。
        inventory::remove(host, &prepared.name, poll, progress)?;
    }

    // tokenの登録はSandboxを消しても残る。Sandboxが消えたあとに解くのは、消し損ねた
    // Sandboxがplaceholderを持ったまま動き続ける状態を作らないためである。commit pointの
    // 前に行うため、失敗したときは案件が管理下に残り、同じcommandでやり直せる。
    secret::forget_github(host, prepared.name.as_str())?;

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
        warnings.push(Warning::text(msg!(
            "warning-lock-file-left-behind",
            path = paths::display(&lock_file),
            detail = error
        )));
    }

    Ok(DestroyOutcome {
        project: prepared.plan.project.clone(),
        re_register: prepared.plan.re_register.clone(),
        warnings,
    })
}

/// Sandboxが存在しないことを1回確認する。
fn require_absent(host: &dyn HostEnvironment, name: &SandboxName) -> Result<()> {
    if inventory::single(&daemon::list(host)?, name.as_str())?.is_some() {
        return Err(inventory::still_present(name));
    }
    Ok(())
}

/// 削除対象。
///
/// pathと同じく、そこに何かがある場合に消すものとして並べる。存在の有無で行を出し
/// 分けると、確認の前に見せる内容がhostへの問い合わせの成否に左右される。
fn removes(paths: &ProjectPaths, name: &SandboxName, state: ProjectState) -> Vec<Target> {
    let mut removes = Vec::new();
    if state != ProjectState::NotCreated {
        removes.push(Target::Described(msg!(
            "destroy-target-sandbox",
            sandbox = name
        )));
    }
    removes.push(Target::Described(msg!(
        "destroy-target-secret",
        sandbox = name,
        env = secret::GITHUB_TOKEN_ENV
    )));
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
///
/// 起点branchのないdetached modeは再現できない。案内できない構成を、実行すると
/// 別の結果になるcommandとして見せない。
fn re_register(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Result<String> {
    let provisioning = &metadata.provisioning;
    // 登録時と同じclone URLを示す。transportを暗黙に変えるcommandを案内しない。
    let command = format!(
        "sbxm add {} --worktrees {}",
        metadata.repository.clone_url(),
        provisioning.requested_worktrees
    );
    match provisioning.mode {
        CreationMode::Attached => Ok(command),
        CreationMode::Detached => match provisioning.start_ref.as_deref() {
            Some(branch) => Ok(format!("{command} --detach {branch}")),
            None => Err(Error::new(
                ErrorId::MetadataInvalidValue,
                msg!(
                    "error-metadata-invalid-value",
                    path = paths::display(&paths.metadata_file()),
                    field = "provisioning.start_ref",
                    detail = "detached mode requires an explicit start branch"
                ),
            )),
        },
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

/// 共通promptで1行を読む対話実装。
///
/// yes/noでは削除しない。完全一致だけを続行の合図とするため、選択一覧ではなく
/// 自由入力を使う。EscとCtrl-Cはどちらも何も変更せず終える。
pub struct TerminalConfirmPrompt {
    prompt: PromptUi,
}

impl TerminalConfirmPrompt {
    pub fn new(prompt: PromptUi) -> TerminalConfirmPrompt {
        TerminalConfirmPrompt { prompt }
    }
}

/// 管理解除の最後に、対応するregistry entryだけを削除する。
///
/// registry entryを先に消して案件を発見不能にしない。project lockを手放したあとで
/// 短時間だけregistry lockを取るため、2つのlockを同時には保持しない。
///
/// どちらのlockも持たない隙間に、同じ案件の`add`が登録をやり直していることがある。
/// canonical project IDの一致だけを根拠にentryを消さず、この実行がcommitした管理解除が
/// registry lockの下でも維持されていることを確かめる。再登録を観測したentryは、
/// 消さずに警告として報告する。
pub fn unregister(
    location: &ConfigLocation,
    unregistration: &Unregistration,
) -> Result<Option<Warning>> {
    let mut guard = RegistryGuard::acquire(location)?;
    let canonical = unregistration.canonical_id();
    let Some(entry) = guard.registry().find(canonical) else {
        return Ok(None);
    };
    if !still_unmanaged(entry, unregistration)? {
        return Ok(Some(registered_again(entry)));
    }
    guard.remove(canonical)?;
    Ok(None)
}

/// この実行が解いた管理が、registry lockの下でも維持されているか。
///
/// 別のrootや別のclone URLへ登録し直されたentryは、この実行が消したentryではない。
/// metadataが戻っていれば案件は再び管理下にあり、entryは新しい登録意図を指している。
/// 観測できなければ、維持されていると推測しない。
fn still_unmanaged(entry: &RegistryEntry, unregistration: &Unregistration) -> Result<bool> {
    if entry.project_root() != unregistration.paths.root()
        || !entry.repository().same_target(&unregistration.repository)
    {
        return Ok(false);
    }
    let root = unregistration.paths.root();
    match std::fs::symlink_metadata(root) {
        // rootごと無くなっていれば、再登録は起きていない。
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(error) => return Err(PathScope::ProjectPath.unreadable_error(root, &error.to_string())),
        Ok(_) => {}
    }
    paths::require_owned_directory(root, PathScope::ProjectPath)?;
    Ok(metadata::load(&unregistration.paths)?.is_none())
}

/// 管理を解いているあいだに登録し直された案件を、entryを残したまま報告する。
fn registered_again(entry: &RegistryEntry) -> Warning {
    Warning::text(msg!(
        "warning-project-registered-again",
        project = entry.repository().display_id(),
        path = paths::display(entry.project_root())
    ))
}

impl ConfirmPrompt for TerminalConfirmPrompt {
    fn confirm_sandbox_name(&mut self, expected: &str) -> Result<bool> {
        let typed = self.prompt.exact(&msg!("destroy-confirm-prompt"))?;
        Ok(typed.trim() == expected)
    }
}

/// 削除して良いことを利用者に確かめる。
///
/// force modeと非対話では対話確認を行わない。TTYの通常modeだけがSandbox名の完全
/// 入力を求め、一致しなければ何も削除しない。
pub fn confirm(
    prepared: &Prepared,
    interactive: bool,
    prompt: &mut dyn ConfirmPrompt,
) -> Result<()> {
    if prepared.force || !interactive {
        return Ok(());
    }
    if prompt.confirm_sandbox_name(&prepared.plan.sandbox)? {
        return Ok(());
    }
    Err(confirmation_mismatch(&prepared.plan.sandbox))
}

/// 入力が一致しない場合のerror。
fn confirmation_mismatch(expected: &str) -> Error {
    Error::new(
        ErrorId::DestroyNotConfirmed,
        msg!("error-destroy-not-confirmed", sandbox = expected),
    )
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

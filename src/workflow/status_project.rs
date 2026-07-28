//! `sbxm status <owner>/<repository>`。
//!
//! 1案件の構築状態、作業可能性、credential隔離をread-onlyで診断する。repair、起動、
//! 停止、file更新を行わない。作成元やsbxm独自のmarkerは検査せず、現在の状態だけを見る。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::hash::{sha256_hex, short_hex};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use super::image::{self, LABEL_CANONICAL_ID, LABEL_DOCKERFILE_SHA256};
use super::inventory::{self, ProjectState};
use super::{daemon, sandbox, worktree};

/// project scopeの状態値。翻訳しない安定したenum。
///
/// `unknown`は使用しない。観測していない項目は、観測できなかった理由を持つ値で示す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Ready,
    Missing,
    Mismatch,
    Changed,
    Running,
    Stopped,
    NotCreated,
    Clean,
    Dirty,
    Attached,
    Detached,
    NotExposed,
    Exposed,
    NotApplicable,
    NotObservedStopped,
}

impl Value {
    pub fn as_str(self) -> &'static str {
        match self {
            Value::Ready => "ready",
            Value::Missing => "missing",
            Value::Mismatch => "mismatch",
            Value::Changed => "changed",
            Value::Running => "running",
            Value::Stopped => "stopped",
            Value::NotCreated => "not-created",
            Value::Clean => "clean",
            Value::Dirty => "dirty",
            Value::Attached => "attached",
            Value::Detached => "detached",
            Value::NotExposed => "not-exposed",
            Value::Exposed => "exposed",
            Value::NotApplicable => "not-applicable",
            Value::NotObservedStopped => "not-observed-stopped",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            Value::Ready => "legend-ready",
            Value::Missing => "legend-missing",
            Value::Mismatch => "legend-mismatch",
            Value::Changed => "legend-changed",
            Value::Running => "legend-sandbox-running",
            Value::Stopped => "legend-sandbox-stopped",
            Value::NotCreated => "legend-not-created",
            Value::Clean => "legend-clean",
            Value::Dirty => "legend-dirty",
            Value::Attached => "legend-attached",
            Value::Detached => "legend-detached",
            Value::NotExposed => "legend-not-exposed",
            Value::Exposed => "legend-exposed",
            Value::NotApplicable => "legend-not-applicable",
            Value::NotObservedStopped => "legend-not-observed-stopped",
        }
    }
}

/// 1件の項目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 項目名のFTL message ID。
    pub item: &'static str,
    pub value: Value,
}

/// worktree 1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRow {
    pub path: String,
    /// metadataとの対応。翻訳しない。
    pub kind: &'static str,
    pub mode: Value,
    pub state: Value,
}

/// 診断結果。
#[derive(Debug, Clone)]
pub struct ProjectStatus {
    pub project: String,
    pub items: Vec<Item>,
    pub worktrees: Vec<WorktreeRow>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ProjectStatus {
    pub fn is_healthy(&self) -> bool {
        self.diagnostics.is_empty()
    }

    fn push(&mut self, item: &'static str, value: Value) {
        self.items.push(Item { item, value });
    }

    /// global環境を読めなかったため観測できなかったことを、別commandの案内とともに残す。
    fn global_scope_failure(&mut self, error: &Error) {
        self.diagnostics.extend(error.diagnostics().iter().cloned());
        self.diagnostics.push(
            Diagnostic::new(
                ErrorId::GlobalScopeUnobservable,
                msg!("error-global-scope-unobservable"),
            )
            .remediation(msg!(
                "remediation-run-global-status",
                command = "sbxm status --global"
            )),
        );
    }
}

/// 1案件を診断する。何も変更しない。
pub fn diagnose(
    config: &GlobalConfig,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<ProjectStatus> {
    let canonical = project.canonical();
    let paths = ProjectPaths::derive(&config.base_path, &canonical);
    let Some(metadata) = crate::metadata::load(&paths)? else {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::ProjectNotManaged,
                msg!("error-project-not-managed", project = project),
            )
            .remediation(msg!(
                "remediation-project-not-managed",
                command = format!("sbxm add {project}")
            )),
        ));
    };
    let name = SandboxName::derive(&canonical);

    let mut status = ProjectStatus {
        project: metadata.display_id(),
        items: Vec::new(),
        worktrees: Vec::new(),
        diagnostics: Vec::new(),
    };

    // 1. metadataと目標構成
    status.push("status-item-metadata", Value::Ready);

    // 2. project rootとhost clone
    check_directory(&paths, &mut status);

    // 3. Dockerfileの世代
    check_dockerfile(&paths, &metadata, &mut status);

    // 4-5. image、archive、Sandbox
    check_image(host, &name, &metadata, &mut status);
    check_archive(&paths, &metadata, &mut status);
    let state = check_sandbox(host, &metadata, workspace_root, &mut status);

    // 6-10. Sandbox内部の検査
    check_inside(host, &name, &metadata, state, &mut status);

    Ok(status)
}

/// project rootとhost cloneの有無。
fn check_directory(paths: &ProjectPaths, status: &mut ProjectStatus) {
    status.push(
        "status-item-project-root",
        if paths.root().is_dir() {
            Value::Ready
        } else {
            Value::Missing
        },
    );
    status.push(
        "status-item-host-clone",
        if paths.host_clone().join(".git").exists() {
            Value::Ready
        } else {
            Value::Missing
        },
    );
}

/// 現在のDockerfileと、metadataに記録した適用済み世代の関係。
fn check_dockerfile(paths: &ProjectPaths, metadata: &ProjectMetadata, status: &mut ProjectStatus) {
    let path = paths.dockerfile();
    match paths::regular_file_exists(&path, PathScope::ProjectPath) {
        Ok(true) => match std::fs::read(&path) {
            Ok(contents) => {
                let digest = sha256_hex(&contents);
                // 変更済みは次の`rebuild`対象であり、破損ではない。
                let value = if digest == metadata.provisioning.dockerfile_sha256 {
                    Value::Ready
                } else {
                    Value::Changed
                };
                status.push("status-item-dockerfile", value);
            }
            Err(error) => {
                status.push("status-item-dockerfile", Value::Mismatch);
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::ProjectPathUnreadable,
                    msg!(
                        "error-project-path-unreadable",
                        path = paths::display(&path),
                        detail = error
                    ),
                ));
            }
        },
        Ok(false) => status.push("status-item-dockerfile", Value::Missing),
        Err(error) => {
            status.push("status-item-dockerfile", Value::Mismatch);
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
        }
    }
}

/// 適用済み世代のimageが、この案件のものとして存在するか。
fn check_image(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    status: &mut ProjectStatus,
) {
    let generation = &metadata.provisioning.dockerfile_sha256;
    let image = image::image_name(name, generation);

    // imageがないことと、Docker Engineへ問い合わせられないことを同じ値へ丸めない。
    let value = match image::inspect(host, &image) {
        Ok(Some(identity)) => {
            let declares_project =
                identity.labels.get(LABEL_CANONICAL_ID) == Some(&metadata.canonical_id.to_string());
            let declares_generation =
                identity.labels.get(LABEL_DOCKERFILE_SHA256) == Some(generation);
            if declares_project && declares_generation {
                Value::Ready
            } else {
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::ImageUnusable,
                    msg!(
                        "error-image-unusable",
                        image = image,
                        detail = "the labels do not declare this project and generation"
                    ),
                ));
                Value::Mismatch
            }
        }
        Ok(None) => Value::Missing,
        Err(error) => {
            status.global_scope_failure(&error);
            Value::Mismatch
        }
    };
    status.push("status-item-image", value);
}

/// 適用済み世代のTemplate archive。
fn check_archive(paths: &ProjectPaths, metadata: &ProjectMetadata, status: &mut ProjectStatus) {
    let archive = paths.template_archive(short_hex(&metadata.provisioning.dockerfile_sha256));
    let value = match paths::regular_file_exists(&archive, PathScope::ProjectPath) {
        Ok(true) => Value::Ready,
        Ok(false) => Value::Missing,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-template-archive", value);
}

/// Sandboxとworkspaceの状態。
///
/// 対象案件だけを名前の完全一致で突き合わせる。ほかの案件の破損で、この案件の状態が
/// 読めなくなることはない。
fn check_sandbox(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
    status: &mut ProjectStatus,
) -> Option<ProjectState> {
    let observed = match daemon::list(host) {
        Ok(entries) => inventory::state_of(&entries, metadata, workspace_root),
        Err(error) => {
            // 一覧そのものを読めないのはglobal環境の問題である。
            status.push("status-item-sandbox", Value::Mismatch);
            status.push("status-item-workspace", Value::Mismatch);
            status.global_scope_failure(&error);
            return None;
        }
    };

    match observed {
        Ok(state) => {
            let (sandbox, workspace) = match state {
                ProjectState::Running => (Value::Running, Value::Ready),
                ProjectState::Stopped => (Value::Stopped, Value::Ready),
                ProjectState::NotCreated => (Value::NotCreated, Value::NotApplicable),
            };
            status.push("status-item-sandbox", sandbox);
            status.push("status-item-workspace", workspace);
            Some(state)
        }
        Err(error) => {
            status.push("status-item-sandbox", Value::Mismatch);
            status.push("status-item-workspace", Value::Mismatch);
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            None
        }
    }
}

/// Sandbox内部の検査。
///
/// Sandboxがない場合は`not-applicable`、停止中は状態を変えないため検査せず
/// `not-observed-stopped`とする。状態そのものを観測できなかった場合は、Sandboxが
/// 無いことにせず`mismatch`とする。
fn check_inside(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    state: Option<ProjectState>,
    status: &mut ProjectStatus,
) {
    let inner = [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ];
    let uniform = match state {
        Some(ProjectState::NotCreated) => Some(Value::NotApplicable),
        // read-onlyの検査でもSandboxを起動し得るため実行しない。
        Some(ProjectState::Stopped) => Some(Value::NotObservedStopped),
        // 観測できなかった状態を、Sandboxが無いことへ丸めない。
        None => Some(Value::Mismatch),
        Some(ProjectState::Running) => None,
    };
    if let Some(value) = uniform {
        for item in inner {
            status.push(item, value);
        }
        return;
    }

    check_secret(host, name, status);
    let layout = SandboxLayout::new(&metadata.canonical_id);
    check_bare_repository(host, name, &layout, status);
    check_worktrees(host, name, &layout, metadata, status);
    check_ssh_agent(host, name, status);
}

fn check_secret(host: &dyn HostEnvironment, name: &SandboxName, status: &mut ProjectStatus) {
    // 登録されていることと、そのSandboxが受け取っていることは別である。片方だけを見て
    // 使える状態とは言えない。
    let value = match super::secret::require_github(host, name.as_str())
        .and_then(|()| super::secret::require_placeholder_present(host, name.as_str()))
    {
        Ok(()) => Value::Ready,
        Err(error) if error.contains_id(ErrorId::GithubSecretMissing) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Missing
        }
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-secret", value);
}

fn check_bare_repository(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    layout: &SandboxLayout,
    status: &mut ProjectStatus,
) {
    let git_dir = layout.bare_git_dir();
    let outcome = sandbox::exec(
        host,
        name.as_str(),
        &[
            "git",
            "--git-dir",
            &git_dir,
            "rev-parse",
            "--is-bare-repository",
        ],
    );
    let value = match outcome {
        Ok(outcome) => match sandbox::inner_exit_code(&outcome) {
            Some(0) if outcome.stdout_text().trim() == "true" => Value::Ready,
            Some(0) => {
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::SandboxRepositoryUnusable,
                    msg!(
                        "error-sandbox-repository-unusable",
                        path = git_dir,
                        detail = "the shared repository is not bare"
                    ),
                ));
                Value::Mismatch
            }
            // `git`がrepositoryとして扱えない場合の終了statusだけを不在とする。
            Some(sandbox::GIT_FATAL) => Value::Missing,
            _ => {
                status.diagnostics.extend(
                    unobservable(&outcome, &git_dir)
                        .diagnostics()
                        .iter()
                        .cloned(),
                );
                Value::Mismatch
            }
        },
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-bare-repository", value);
}

/// Sandbox内のworktreeを、metadataと突き合わせて分類する。
fn check_worktrees(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
    status: &mut ProjectStatus,
) {
    let entries = match worktree::list(host, name.as_str(), layout) {
        Ok(entries) => entries,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            status.push("status-item-worktrees", Value::Mismatch);
            return;
        }
    };

    let bare_root = layout.bare_root();
    let mut seen: Vec<String> = Vec::new();
    let mut value = Value::Ready;
    for entry in entries {
        if entry.bare {
            // bare entryはworktree数へ含めない。
            continue;
        }
        let Some(relative) = entry.relative_to(&bare_root) else {
            // bare root配下へstandardizeできないpathは、案件の成果物として扱わない。
            status.diagnostics.push(Diagnostic::new(
                ErrorId::SandboxRepositoryUnusable,
                msg!(
                    "error-sandbox-repository-unusable",
                    path = entry.path,
                    detail = "the worktree is outside the shared repository"
                ),
            ));
            value = Value::Mismatch;
            continue;
        };
        let managed = metadata
            .managed_worktrees
            .iter()
            .any(|worktree| worktree.path == relative);
        seen.push(relative.clone());

        let mode = if entry.detached {
            Value::Detached
        } else {
            Value::Attached
        };
        let state = worktree_state(host, name, &entry.path, status);
        if state == Value::Mismatch {
            value = Value::Mismatch;
        }
        status.worktrees.push(WorktreeRow {
            path: relative,
            kind: if managed { "managed" } else { "unmanaged" },
            mode,
            state,
        });
    }

    for declared in &metadata.managed_worktrees {
        if !seen.contains(&declared.path) {
            status.diagnostics.push(Diagnostic::new(
                ErrorId::SandboxRepositoryUnusable,
                msg!(
                    "error-sandbox-repository-unusable",
                    path = declared.path,
                    detail =
                        "the metadata declares this managed worktree, but Git does not have it"
                ),
            ));
            status.worktrees.push(WorktreeRow {
                path: declared.path.clone(),
                kind: "managed",
                mode: Value::Mismatch,
                state: Value::Mismatch,
            });
            value = Value::Mismatch;
        }
    }
    status.push("status-item-worktrees", value);
}

/// 作業中の変更があるか。submoduleの変更も`git status`が示すとおりに扱う。
fn worktree_state(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    path: &str,
    status: &mut ProjectStatus,
) -> Value {
    match sandbox::exec(
        host,
        name.as_str(),
        &[
            "git",
            "-C",
            path,
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
        ],
    ) {
        Ok(outcome) if outcome.success() => {
            if outcome
                .stdout_text()
                .trim_matches(['\0', '\n', ' '])
                .is_empty()
            {
                Value::Clean
            } else {
                Value::Dirty
            }
        }
        Ok(outcome) => {
            status
                .diagnostics
                .extend(unobservable(&outcome, path).diagnostics().iter().cloned());
            Value::Mismatch
        }
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    }
}

/// SSH Agentが露出していないこと。
///
/// 露出していないことは、検査commandが答えた場合にだけ言える。検査自体が成立しない
/// 場合を`not-exposed`へ丸めない。
fn check_ssh_agent(host: &dyn HostEnvironment, name: &SandboxName, status: &mut ProjectStatus) {
    let value = match sandbox::ssh_agent_is_exposed(host, name.as_str()) {
        Ok(observed) if !observed.is_empty() => {
            status.diagnostics.push(
                Diagnostic::new(
                    ErrorId::SshAgentExposed,
                    msg!(
                        "security-ssh-agent-exposed-description",
                        sandbox = name,
                        observed = observed.join(", ")
                    ),
                )
                .remediation(msg!(
                    "security-ssh-agent-exposed-remediation",
                    command = format!("sbx rm {name}")
                )),
            );
            Value::Exposed
        }
        Ok(_) => Value::NotExposed,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-ssh-agent", value);
}

/// 内側のcommandが答えなかった場合の診断。原値をそのまま残す。
fn unobservable(outcome: &crate::command::CommandOutcome, subject: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxCheckUnobservable,
            msg!(
                "error-sandbox-check-unobservable",
                subject = subject,
                exit_status = outcome.status
            ),
        )
        .external(outcome.failure()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::inventory::tests::{FakeSbx, Registered, fixture};

    fn value_of(status: &ProjectStatus, item: &str) -> Value {
        status
            .items
            .iter()
            .find(|entry| entry.item == item)
            .unwrap_or_else(|| panic!("item {item} is missing"))
            .value
    }

    fn project_id(value: &str) -> ProjectId {
        ProjectId::parse(value).expect("valid project id")
    }

    /// imageがまだ存在しないhost。一覧は答えるが、1件も返さない。
    fn without_image(host: FakeSbx, project: &Registered) -> FakeSbx {
        let image = image::image_name(
            &project.sandbox,
            &project.metadata.provisioning.dockerfile_sha256,
        );
        host.answering(&format!("image ls --quiet {image}"), 0, "")
    }

    #[test]
    fn a_project_that_is_not_managed_cannot_be_diagnosed() {
        let fixture = fixture();
        let host = FakeSbx::listing("[]");
        let error = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect_err("there is nothing to diagnose");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    }

    #[test]
    fn the_items_are_reported_in_the_documented_order() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let host = without_image(FakeSbx::listing("[]"), &project);

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");

        assert_eq!(
            status
                .items
                .iter()
                .map(|item| item.item)
                .collect::<Vec<_>>(),
            vec![
                "status-item-metadata",
                "status-item-project-root",
                "status-item-host-clone",
                "status-item-dockerfile",
                "status-item-image",
                "status-item-template-archive",
                "status-item-sandbox",
                "status-item-workspace",
                "status-item-secret",
                "status-item-bare-repository",
                "status-item-worktrees",
                "status-item-ssh-agent",
            ]
        );
    }

    #[test]
    fn a_project_without_a_sandbox_reports_the_inner_items_as_not_applicable() {
        let fixture = fixture();
        let project = fixture.register("Example-Org/Example-Repo");
        let host = without_image(FakeSbx::listing("[]"), &project);

        let status = diagnose(
            &fixture.config,
            &project_id("Example-Org/Example-Repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");

        assert_eq!(status.project, "Example-Org/Example-Repo");
        assert_eq!(value_of(&status, "status-item-metadata"), Value::Ready);
        assert_eq!(value_of(&status, "status-item-sandbox"), Value::NotCreated);
        for item in [
            "status-item-secret",
            "status-item-bare-repository",
            "status-item-worktrees",
            "status-item-ssh-agent",
        ] {
            assert_eq!(value_of(&status, item), Value::NotApplicable, "{item}");
        }
        assert!(status.worktrees.is_empty());
    }

    #[test]
    fn a_stopped_sandbox_is_not_started_to_look_inside_it() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let host = without_image(
            FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "stopped"))),
            &project,
        );

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");

        assert_eq!(value_of(&status, "status-item-sandbox"), Value::Stopped);
        assert_eq!(
            value_of(&status, "status-item-ssh-agent"),
            Value::NotObservedStopped
        );
        assert_eq!(
            value_of(&status, "status-item-worktrees"),
            Value::NotObservedStopped
        );
        assert!(
            !host.ran("exec"),
            "nothing runs inside a stopped sandbox: {:?}",
            host.calls()
        );
        assert!(
            status.is_healthy(),
            "not observing on purpose is not a failure"
        );
    }

    #[test]
    fn a_sandbox_state_that_cannot_be_read_is_not_reported_as_a_missing_sandbox() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        // 一覧を読めない状態でも、取得できた項目は表示する。
        let host = without_image(FakeSbx::listing("not json"), &project);

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");

        assert_eq!(value_of(&status, "status-item-metadata"), Value::Ready);
        assert_eq!(value_of(&status, "status-item-sandbox"), Value::Mismatch);
        for item in [
            "status-item-secret",
            "status-item-bare-repository",
            "status-item-worktrees",
            "status-item-ssh-agent",
        ] {
            assert_eq!(
                value_of(&status, item),
                Value::Mismatch,
                "{item} is not observed, which is not the same as absent"
            );
        }
        assert!(
            status
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.id == ErrorId::GlobalScopeUnobservable),
            "the global command that can diagnose this is named: {:?}",
            status.diagnostics
        );
        assert!(!host.ran("exec"), "nothing runs inside an unknown sandbox");
    }

    #[test]
    fn an_unrelated_project_does_not_decide_this_one() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        // 別案件のmetadataが壊れていても、この案件の状態は読める。
        let broken = fixture
            .config
            .base_path
            .as_path()
            .join("broken/broken.project/.sbxm");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("project.toml"), "version = 2\n").unwrap();

        let host = without_image(
            FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "stopped"))),
            &project,
        );
        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");
        assert_eq!(value_of(&status, "status-item-sandbox"), Value::Stopped);
    }

    #[test]
    fn an_engine_that_cannot_be_asked_does_not_make_an_image_absent() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let image = image::image_name(
            &project.sandbox,
            &project.metadata.provisioning.dockerfile_sha256,
        );
        let host = FakeSbx::listing("[]").answering(&format!("image ls --quiet {image}"), 1, "");

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");
        assert_eq!(value_of(&status, "status-item-image"), Value::Mismatch);
        assert!(
            status
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.id == ErrorId::GlobalScopeUnobservable),
            "the engine that could not answer is named: {:?}",
            status.diagnostics
        );
    }

    #[test]
    fn a_changed_dockerfile_is_reported_as_the_next_rebuild_rather_than_a_fault() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
        let host = without_image(FakeSbx::listing("[]"), &project);

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");
        assert_eq!(value_of(&status, "status-item-dockerfile"), Value::Changed);
        assert!(status.is_healthy(), "a changed Dockerfile is not an error");
    }

    #[test]
    fn a_running_sandbox_is_looked_into_and_its_worktrees_classified() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let listing = format!("[{}]", fixture.entry(&project, "running"));
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let managed = format!("{}/example-repo.tree-0", layout.bare_root());
        let unmanaged = format!("{}/agent-scratch", layout.bare_root());

        let host = FakeSbx::listing(&listing)
            .answering(
                &format!(
                    "exec {} -- git --git-dir {} rev-parse --is-bare-repository",
                    project.sandbox,
                    layout.bare_git_dir()
                ),
                0,
                "true\n",
            )
            .answering(
                &format!(
                    "exec {} -- git --git-dir {} worktree list --porcelain -z",
                    project.sandbox,
                    layout.bare_git_dir()
                ),
                0,
                &format!(
                    "worktree {}\0bare\0\0worktree {managed}\0branch refs/heads/main\0\0worktree {unmanaged}\0detached\0\0",
                    layout.bare_root()
                ),
            )
            .answering(
                &format!(
                    "exec {} -- git -C {unmanaged} status --porcelain=v2 -z --untracked-files=all",
                    project.sandbox
                ),
                0,
                "1 .M N... 100644 100644 100644 abc abc file.txt\0",
            )
            .answering(
                &format!("secret ls {}", project.sandbox),
                0,
                &format!(
                    "CUSTOM SECRETS\nSCOPE   TARGETS   ENV   PLACEHOLDER   SECRET\nx   {}   GH_TOKEN   sbx-cs-example   ghp_example\n",
                    crate::workflow::secret::GITHUB_HOSTS.join(" ")
                ),
            )
            .answering(
                &format!(
                    "exec {} -- sh -c {}",
                    project.sandbox,
                    super::super::secret::placeholder_probe()
                ),
                0,
                "sbx-cs-example",
            )
            .answering(
                &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
                1,
                "",
            )
            .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 2, "");

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");

        assert_eq!(value_of(&status, "status-item-sandbox"), Value::Running);
        assert_eq!(value_of(&status, "status-item-secret"), Value::Ready);
        assert_eq!(
            value_of(&status, "status-item-bare-repository"),
            Value::Ready
        );
        assert_eq!(
            value_of(&status, "status-item-ssh-agent"),
            Value::NotExposed
        );
        assert_eq!(
            status.worktrees,
            vec![
                WorktreeRow {
                    path: "example-repo.tree-0".to_string(),
                    kind: "managed",
                    mode: Value::Attached,
                    state: Value::Clean,
                },
                WorktreeRow {
                    path: "agent-scratch".to_string(),
                    kind: "unmanaged",
                    mode: Value::Detached,
                    state: Value::Dirty,
                },
            ]
        );
    }

    #[test]
    fn an_ssh_agent_inside_the_sandbox_is_a_security_failure() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let listing = format!("[{}]", fixture.entry(&project, "running"));
        let host = FakeSbx::listing(&listing)
            .answering(
                &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
                0,
                "/tmp/ssh-agent.sock\n",
            )
            .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 2, "");

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");

        assert_eq!(value_of(&status, "status-item-ssh-agent"), Value::Exposed);
        assert!(
            status
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.id == ErrorId::SshAgentExposed)
        );
        assert!(!status.is_healthy());
    }

    #[test]
    fn an_agent_that_answers_without_keys_is_still_reachable() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let listing = format!("[{}]", fixture.entry(&project, "running"));
        // socketは未設定でも、agentへ接続できる時点で露出している。
        let host = FakeSbx::listing(&listing)
            .answering(
                &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
                1,
                "",
            )
            .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 1, "");

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");
        assert_eq!(value_of(&status, "status-item-ssh-agent"), Value::Exposed);
    }

    #[test]
    fn a_check_that_could_not_run_is_not_read_as_not_exposed() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let listing = format!("[{}]", fixture.entry(&project, "running"));
        // command不在は、露出していないことの証明にならない。
        let host = FakeSbx::listing(&listing)
            .answering(
                &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
                127,
                "",
            )
            .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 127, "");

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");
        assert_eq!(value_of(&status, "status-item-ssh-agent"), Value::Mismatch);
        assert!(
            status
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.id == ErrorId::SandboxCheckUnobservable)
        );
        assert!(!status.is_healthy(), "an unprovable check is not a pass");
    }

    #[test]
    fn a_repository_check_that_could_not_run_is_not_read_as_missing() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let listing = format!("[{}]", fixture.entry(&project, "running"));
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let host = FakeSbx::listing(&listing).answering(
            &format!(
                "exec {} -- git --git-dir {} rev-parse --is-bare-repository",
                project.sandbox,
                layout.bare_git_dir()
            ),
            127,
            "",
        );

        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");
        assert_eq!(
            value_of(&status, "status-item-bare-repository"),
            Value::Mismatch
        );

        // repositoryとして扱えないことは、gitが答えた結果なので不在とする。
        let host = FakeSbx::listing(&listing).answering(
            &format!(
                "exec {} -- git --git-dir {} rev-parse --is-bare-repository",
                project.sandbox,
                layout.bare_git_dir()
            ),
            128,
            "",
        );
        let status = diagnose(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
        )
        .expect("diagnose");
        assert_eq!(
            value_of(&status, "status-item-bare-repository"),
            Value::Missing
        );
    }
}

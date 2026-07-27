//! Sandboxの作成と再利用。
//!
//! Workspaceは案件pathともhomeとも無関係な中立pathとし、host側の実pathを
//! Sandboxへ公開しない。既存Sandboxは、作成元を問わず、期待する状態と一致することを
//! 観測できた場合だけ再利用する。

use std::fs;
use std::path::{Path, PathBuf};

use crate::command::{CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::{SandboxEntry, SandboxState};
use crate::error::{Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope};
use crate::project::SandboxName;

use super::daemon;
use super::template::LoadedTemplate;

/// 中立Workspaceのroot。
pub const WORKSPACE_ROOT: &str = "/tmp/docker-sandboxes";

/// Sandboxへ渡すagent kit。対話shellを持つ最小構成を使う。
const AGENT_KIT: &str = "shell";

/// 使用できる状態のSandbox。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadySandbox {
    pub name: String,
    pub workspace: PathBuf,
    pub state: SandboxState,
    /// この実行で作成したか。
    pub created: bool,
}

/// `<workspace-root>/<sandbox-name>`
pub fn workspace_path(root: &Path, sandbox: &SandboxName) -> PathBuf {
    root.join(sandbox.as_str())
}

/// Sandboxを用意する。
///
/// 呼び出し側はdaemonの安全性を確認した区間で呼ぶ。
pub fn ensure(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    template: &LoadedTemplate,
    workspace_root: &Path,
) -> Result<ReadySandbox> {
    let workspace = workspace_path(workspace_root, sandbox);
    paths::ensure_private_dir(&workspace, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;

    if let Some(entry) = find(host, sandbox)? {
        verify(&entry, sandbox, template, &workspace)?;
        return Ok(ReadySandbox {
            name: entry.name,
            workspace,
            state: entry.state,
            created: false,
        });
    }

    let spec = CommandSpec::passthrough(
        "sbx",
        &[
            "create",
            "--name",
            sandbox.as_str(),
            "--template",
            &template.name,
            AGENT_KIT,
            &paths::display(&workspace),
        ],
    )
    .env(EnvPolicy::InheritWithoutSshAgent)
    .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;

    let Some(entry) = find(host, sandbox)? else {
        return Err(unusable(
            sandbox.as_str(),
            "the sandbox is absent right after it was created".to_string(),
        ));
    };
    verify(&entry, sandbox, template, &workspace)?;

    Ok(ReadySandbox {
        name: entry.name,
        workspace,
        state: entry.state,
        created: true,
    })
}

/// Sandbox内でcommandを実行する。
///
/// 引数配列のまま渡し、shellを介さない。出力はparseまたは秘匿のためcaptureする。
pub fn exec(host: &dyn HostEnvironment, sandbox: &str, args: &[&str]) -> Result<CommandOutcome> {
    run_exec(host, sandbox, None, args)
}

/// Sandbox内でrootとしてcommandを実行する。
pub fn exec_as_root(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
) -> Result<CommandOutcome> {
    run_exec(host, sandbox, Some("root"), args)
}

/// Sandbox内で、進捗をそのまま見せるcommandを実行する。
///
/// cloneやfetchのように、時間のかかる工程の進捗を実行中に見せるために使う。
pub fn exec_with_progress(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
) -> Result<CommandOutcome> {
    let full = exec_arguments(sandbox, None, args);
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    let spec = CommandSpec::passthrough("sbx", &borrowed)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::RepositoryTransfer);
    host.run(&spec)
}

fn run_exec(
    host: &dyn HostEnvironment,
    sandbox: &str,
    user: Option<&str>,
    args: &[&str],
) -> Result<CommandOutcome> {
    let full = exec_arguments(sandbox, user, args);
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    let spec = CommandSpec::capture("sbx", &borrowed)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)
}

fn exec_arguments(sandbox: &str, user: Option<&str>, args: &[&str]) -> Vec<String> {
    let mut full: Vec<String> = vec!["exec".to_string()];
    if let Some(user) = user {
        full.push("--user".to_string());
        full.push(user.to_string());
    }
    full.push(sandbox.to_string());
    full.push("--".to_string());
    full.extend(args.iter().map(|arg| arg.to_string()));
    full
}

/// 名前が完全一致するSandboxを探す。
///
/// 名前はcanonical project IDから決定的に導出されるため、名前の一致が案件との
/// 対応そのものになる。
fn find(host: &dyn HostEnvironment, sandbox: &SandboxName) -> Result<Option<SandboxEntry>> {
    let sandboxes = daemon::list(host)?;
    Ok(sandboxes
        .into_iter()
        .find(|entry| entry.name == sandbox.as_str()))
}

/// 既存Sandboxが、この案件のものであることをread-onlyで確認する。
pub fn verify_identity(
    entry: &SandboxEntry,
    sandbox: &SandboxName,
    template_name: &str,
    workspace_root: &Path,
) -> Result<()> {
    let template = LoadedTemplate {
        name: template_name.to_string(),
        loaded: false,
    };
    verify(
        entry,
        sandbox,
        &template,
        &workspace_path(workspace_root, sandbox),
    )
}

/// 既存Sandboxが期待する構成であることを確認する。
///
/// 誰が作成したかは条件にしない。1項目でも確認できない場合は再利用しない。
fn verify(
    entry: &SandboxEntry,
    sandbox: &SandboxName,
    template: &LoadedTemplate,
    workspace: &Path,
) -> Result<()> {
    match &entry.workspace {
        Some(observed) => {
            let observed = real_path(Path::new(observed));
            let expected = real_path(workspace);
            if observed != expected {
                return Err(unusable(
                    sandbox.as_str(),
                    format!(
                        "the sandbox works in {}, not in {}",
                        paths::display(&observed),
                        paths::display(&expected)
                    ),
                ));
            }
        }
        None => {
            return Err(unusable(
                sandbox.as_str(),
                "this Docker Sandboxes version does not report the workspace of a sandbox"
                    .to_string(),
            ));
        }
    }

    match &entry.template {
        Some(observed) if *observed == template.name => Ok(()),
        Some(observed) => Err(unusable(
            sandbox.as_str(),
            format!(
                "the sandbox was made from {observed}, not from {}",
                template.name
            ),
        )),
        None => Err(unusable(
            sandbox.as_str(),
            "this Docker Sandboxes version does not report the template of a sandbox".to_string(),
        )),
    }
}

/// symlinkを解決できない場合は宣言されたpathのまま比較する。
fn real_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| paths::lexically_standardize(path))
}

fn unusable(name: &str, detail: String) -> Error {
    Error::new(
        ErrorId::SandboxUnusable,
        msg!("error-sandbox-unusable", sandbox = name, detail = detail),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::project::ProjectId;
    use std::cell::RefCell;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;

    struct FakeSbx {
        listings: RefCell<Vec<String>>,
        calls: RefCell<Vec<CommandSpec>>,
    }

    impl FakeSbx {
        fn listing(outputs: &[&str]) -> FakeSbx {
            FakeSbx {
                listings: RefCell::new(
                    outputs
                        .iter()
                        .rev()
                        .map(|value| value.to_string())
                        .collect(),
                ),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls
                .borrow()
                .iter()
                .map(|spec| spec.args.clone())
                .collect()
        }
    }

    impl HostEnvironment for FakeSbx {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
            self.calls.borrow_mut().push(spec.clone());
            let stdout = if spec.args.first().is_some_and(|arg| arg == "ls") {
                self.listings.borrow_mut().pop().unwrap_or_default()
            } else {
                String::new()
            };
            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(0),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

    fn sandbox() -> SandboxName {
        SandboxName::derive(
            &ProjectId::parse("example-org/example-repo")
                .unwrap()
                .canonical(),
        )
    }

    fn template() -> LoadedTemplate {
        LoadedTemplate {
            name: "sbxm-example-template:111111111111".to_string(),
            loaded: true,
        }
    }

    fn listing(workspace: &Path, template: &str, state: &str) -> String {
        format!(
            r#"[{{"name":"{}","state":"{state}","workspace":"{}","template":"{template}","active_sessions":0}}]"#,
            sandbox(),
            workspace.display()
        )
    }

    #[test]
    fn a_missing_sandbox_is_created_from_the_template_in_a_neutral_workspace() {
        let root = tempfile::tempdir().unwrap();
        let workspace = workspace_path(root.path(), &sandbox());
        let host = FakeSbx::listing(&["[]", &listing(&workspace, &template().name, "running")]);

        let ready = ensure(&host, &sandbox(), &template(), root.path()).expect("create");
        assert!(ready.created);
        assert_eq!(ready.state, SandboxState::Running);
        assert_eq!(ready.workspace, workspace);
        assert_eq!(
            fs::metadata(&workspace).unwrap().permissions().mode() & 0o777,
            PRIVATE_DIR_MODE
        );

        assert_eq!(
            host.calls()[1],
            vec![
                "create".to_string(),
                "--name".to_string(),
                sandbox().as_str().to_string(),
                "--template".to_string(),
                template().name,
                AGENT_KIT.to_string(),
                paths::display(&workspace),
            ]
        );
    }

    #[test]
    fn the_workspace_path_carries_no_project_or_home_path() {
        let workspace = workspace_path(Path::new(WORKSPACE_ROOT), &sandbox());
        assert_eq!(
            workspace,
            Path::new("/tmp/docker-sandboxes").join(sandbox().as_str())
        );
        assert!(!paths::display(&workspace).contains("/Users/"));
    }

    #[test]
    fn a_sandbox_that_matches_the_expected_state_is_reused_whoever_made_it() {
        let root = tempfile::tempdir().unwrap();
        let workspace = workspace_path(root.path(), &sandbox());
        let host = FakeSbx::listing(&[&listing(&workspace, &template().name, "stopped")]);

        let ready = ensure(&host, &sandbox(), &template(), root.path()).expect("reuse");
        assert!(!ready.created);
        assert_eq!(ready.state, SandboxState::Stopped);
        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.first().is_some_and(|arg| arg == "create")),
            "an existing sandbox is never created over"
        );
    }

    #[test]
    fn a_sandbox_with_another_workspace_or_template_stops_the_run() {
        let root = tempfile::tempdir().unwrap();
        let workspace = workspace_path(root.path(), &sandbox());

        let elsewhere = root.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let host = FakeSbx::listing(&[&listing(&elsewhere, &template().name, "running")]);
        let error = ensure(&host, &sandbox(), &template(), root.path())
            .expect_err("a sandbox that works elsewhere is not this project's");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));

        let host = FakeSbx::listing(&[&listing(&workspace, "sbxm-other-template:2", "running")]);
        let error = ensure(&host, &sandbox(), &template(), root.path())
            .expect_err("a sandbox from another template is not reused");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    }

    #[test]
    fn a_runtime_that_hides_the_workspace_or_the_template_is_not_guessed_at() {
        let root = tempfile::tempdir().unwrap();
        for listing in [
            format!(
                r#"[{{"name":"{}","state":"running","template":"{}"}}]"#,
                sandbox(),
                template().name
            ),
            format!(
                r#"[{{"name":"{}","state":"running","workspace":"{}"}}]"#,
                sandbox(),
                workspace_path(root.path(), &sandbox()).display()
            ),
        ] {
            let host = FakeSbx::listing(&[&listing]);
            let error = ensure(&host, &sandbox(), &template(), root.path())
                .expect_err("an unverifiable sandbox is not reused");
            assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
        }
    }

    #[test]
    fn a_workspace_that_is_a_symlink_is_refused_before_anything_is_created() {
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("real");
        fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, workspace_path(root.path(), &sandbox())).unwrap();

        let host = FakeSbx::listing(&["[]"]);
        let error = ensure(&host, &sandbox(), &template(), root.path())
            .expect_err("a symlinked workspace is refused");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
        assert!(host.calls().is_empty(), "nothing is asked of the runtime");
    }
}

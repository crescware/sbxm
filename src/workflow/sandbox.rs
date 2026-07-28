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
///
/// 共有されるdirectoryの下にあるため、rootとその下のworkspaceの両方を、現在の
/// 利用者だけが使えるdirectoryとして検証または作成する。
pub const WORKSPACE_ROOT: &str = "/tmp/docker-sandboxes";

/// Sandboxへ渡すagent kit。対話shellを持つ最小構成を使う。
const AGENT_KIT: &str = "shell";

/// `git`が対象をrepositoryとして扱えなかったときの終了status。
pub const GIT_FATAL: i32 = 128;

/// `ssh-add`がagentへ接続できなかったときの終了status。
///
/// 鍵が1件もない場合は`1`で終わるため、接続できたかどうかとは区別できる。
pub const SSH_ADD_NO_AGENT: i32 = 2;

/// exec自体の失敗を示す終了status。POSIX shellとcontainer runtimeの慣例に従う。
const EXEC_FAILURE: std::ops::RangeInclusive<i32> = 125..=127;

/// Sandbox内で動いたcommand自身の終了status。
///
/// `sbx exec`が内側のcommandを起動できなかった場合、およびsignalで終わった場合は
/// `None`とする。実行できなかったことを、内側のcommandが返した結果として読まない。
pub fn inner_exit_code(outcome: &CommandOutcome) -> Option<i32> {
    match outcome.status.code() {
        Some(code) if !EXEC_FAILURE.contains(&code) => Some(code),
        _ => None,
    }
}

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
    // rootを別accountが所有していると、その下のworkspaceを入れ替えられる。
    paths::ensure_private_dir(workspace_root, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    let workspace = workspace_path(workspace_root, sandbox);
    paths::ensure_private_dir(&workspace, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;

    let templates = [template.name.clone()];
    if let Some(entry) = find(host, sandbox)? {
        verify(&entry, sandbox, &templates, &workspace)?;
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
    verify(&entry, sandbox, &templates, &workspace)?;

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
///
/// `templates`はmetadataが正本とする世代のTemplate名であり、rebuild intent中は
/// 2世代を受け入れる。
pub fn verify_identity(
    entry: &SandboxEntry,
    sandbox: &SandboxName,
    templates: &[String],
    workspace_root: &Path,
) -> Result<()> {
    verify(
        entry,
        sandbox,
        templates,
        &workspace_path(workspace_root, sandbox),
    )
}

/// 既存Sandboxが期待する構成であることを確認する。
///
/// 誰が作成したかは条件にしない。案件との対応は、canonical project IDから導出した
/// 名前と、その案件だけが使う中立Workspaceの実pathで判定する。
///
/// 由来Templateは、runtimeが示す場合だけ照合する。対象versionは一覧にTemplateを
/// 含めないため、示さない一覧からは世代を確かめられない。ここで拒否すると、名前と
/// workspaceが一致しているSandboxを一度も使えなくなる。世代の一致は`rebuild`が
/// 必要とする条件であり、この検査が保証するものではない。
fn verify(
    entry: &SandboxEntry,
    sandbox: &SandboxName,
    templates: &[String],
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
        Some(observed) if templates.iter().any(|expected| expected == observed) => Ok(()),
        Some(observed) => Err(unusable(
            sandbox.as_str(),
            format!(
                "the sandbox was made from {observed}, not from {}",
                templates.join(" or ")
            ),
        )),
        // 一覧がTemplateを持たないversionでは、名前とworkspaceだけを根拠にする。
        None => Ok(()),
    }
}

/// symlinkを解決できない場合は宣言されたpathのまま比較する。
fn real_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| paths::lexically_standardize(path))
}

/// hostのSSH AgentへSandboxの中から到達できるか。
///
/// 露出していないことは、検査commandが答えた場合にだけ言える。検査が成立しなかった
/// 場合を「露出していない」へ丸めず、判定できないerrorとして返す。
pub fn ssh_agent_is_exposed(
    host: &dyn HostEnvironment,
    sandbox: &str,
) -> Result<Vec<&'static str>> {
    let mut observed = Vec::new();

    let socket = exec(host, sandbox, &["printenv", "SSH_AUTH_SOCK"])?;
    match inner_exit_code(&socket) {
        Some(0) if !socket.stdout_text().trim().is_empty() => observed.push("SSH_AUTH_SOCK is set"),
        Some(0) => {}
        // `printenv`は未設定のとき`1`で終わる。
        Some(1) => {}
        _ => return Err(unobservable(&socket, "SSH_AUTH_SOCK")),
    }

    let keys = exec(host, sandbox, &["ssh-add", "-L"])?;
    // 公開鍵本文は読まず、agentへ接続できたかどうかだけを見る。
    match inner_exit_code(&keys) {
        // 鍵の有無にかかわらず、agentへ接続できた時点で露出している。
        Some(0) | Some(1) => observed.push("ssh-add reached an agent"),
        Some(SSH_ADD_NO_AGENT) => {}
        _ => return Err(unobservable(&keys, "ssh-add")),
    }

    Ok(observed)
}

/// 作成または再作成したSandboxが、hostのcredentialから隔離されていること。
pub fn require_credentials_isolated(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let observed = ssh_agent_is_exposed(host, sandbox)?;
    if observed.is_empty() {
        return Ok(());
    }
    Err(Error::single(
        crate::error::Diagnostic::new(
            ErrorId::SshAgentExposed,
            msg!(
                "security-ssh-agent-exposed-description",
                sandbox = sandbox,
                observed = observed.join(", ")
            ),
        )
        .remediation(msg!("security-ssh-agent-exposed-remediation")),
    ))
}

/// 内側のcommandが答えなかった場合の診断。原値をそのまま残す。
pub fn unobservable(outcome: &CommandOutcome, subject: &str) -> Error {
    Error::single(
        crate::error::Diagnostic::new(
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

    /// SSH Agent検査の2つのcommandに、決め打ちの結果を返すhost。
    struct FakeProbe {
        socket: (i32, &'static str),
        keys: (i32, &'static str),
    }

    impl HostEnvironment for FakeProbe {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
            let (code, stdout) = if spec.args.iter().any(|arg| arg == "printenv") {
                self.socket
            } else {
                self.keys
            };
            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(code << 8),
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

    #[test]
    fn a_sandbox_that_cannot_reach_the_host_agent_passes_the_isolation_check() {
        // `printenv`は未設定を`1`で、`ssh-add`はagent不在を`2`で示す。
        let host = FakeProbe {
            socket: (1, ""),
            keys: (SSH_ADD_NO_AGENT, ""),
        };
        assert!(
            ssh_agent_is_exposed(&host, "sandbox")
                .expect("the check answered")
                .is_empty()
        );
        require_credentials_isolated(&host, "sandbox").expect("nothing is exposed");
    }

    #[test]
    fn either_a_socket_or_a_reachable_agent_counts_as_exposed() {
        let by_socket = FakeProbe {
            socket: (0, "/tmp/agent.sock\n"),
            keys: (SSH_ADD_NO_AGENT, ""),
        };
        let by_agent = FakeProbe {
            socket: (1, ""),
            // 鍵が1件もない`1`でも、agentへ接続できている。
            keys: (1, ""),
        };

        for host in [by_socket, by_agent] {
            let error = require_credentials_isolated(&host, "sandbox")
                .expect_err("the host agent is reachable from inside");
            assert_eq!(error.first_id(), Some(ErrorId::SshAgentExposed));
        }
    }

    #[test]
    fn a_probe_that_could_not_run_is_not_read_as_isolation() {
        let host = FakeProbe {
            socket: (126, ""),
            keys: (SSH_ADD_NO_AGENT, ""),
        };
        let error = require_credentials_isolated(&host, "sandbox")
            .expect_err("a check that did not answer never means isolated");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
    }

    /// 中立Workspaceのrootを、実行時と同じ条件で用意する。
    fn workspace_root() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temporary workspace root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(PRIVATE_DIR_MODE))
            .expect("the root belongs to the current user only");
        root
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
        let root = workspace_root();
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
        let root = workspace_root();
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
        let root = workspace_root();
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
    fn a_runtime_that_hides_the_workspace_is_not_guessed_at() {
        let root = workspace_root();
        // workspaceが分からない一覧からは、この案件のSandboxだと言えない。
        let listing = format!(
            r#"[{{"name":"{}","state":"running","template":"{}"}}]"#,
            sandbox(),
            template().name
        );
        let host = FakeSbx::listing(&[&listing]);
        let error = ensure(&host, &sandbox(), &template(), root.path())
            .expect_err("an unverifiable sandbox is not reused");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    }

    #[test]
    fn a_runtime_that_does_not_report_templates_still_identifies_the_sandbox() {
        let root = workspace_root();
        // 対象versionの一覧はTemplateを持たない。名前とworkspaceで対応を判定する。
        let listing = format!(
            r#"[{{"name":"{}","status":"running","workspaces":["{}"]}}]"#,
            sandbox(),
            workspace_path(root.path(), &sandbox()).display()
        );
        let host = FakeSbx::listing(&[&listing]);

        let ready = ensure(&host, &sandbox(), &template(), root.path())
            .expect("the sandbox of this project is used");
        assert!(!ready.created, "an existing sandbox is not made again");
        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.first().is_some_and(|arg| arg == "create")),
            "nothing is created over it: {:?}",
            host.calls()
        );
    }

    #[test]
    fn a_sandbox_made_from_another_template_is_still_refused_when_it_is_reported() {
        let root = workspace_root();
        let listing = format!(
            r#"[{{"name":"{}","status":"running","workspaces":["{}"],"template":"other-template:1"}}]"#,
            sandbox(),
            workspace_path(root.path(), &sandbox()).display()
        );
        let host = FakeSbx::listing(&[&listing]);
        let error = ensure(&host, &sandbox(), &template(), root.path())
            .expect_err("a template the runtime does report is still checked");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    }

    #[test]
    fn a_workspace_that_is_a_symlink_is_refused_before_anything_is_created() {
        let root = workspace_root();
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

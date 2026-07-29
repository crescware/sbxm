//! Sandboxの作成と再利用。
//!
//! Workspaceは案件pathともhomeとも無関係な中立pathとし、host側の実pathを
//! Sandboxへ公開しない。既存Sandboxは、作成元を問わず、期待する状態と一致することを
//! 観測できた場合だけ再利用する。

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

    if let Some(entry) = find(host, sandbox)? {
        verify(&entry, sandbox, &workspace)?;
        return Ok(ReadySandbox {
            name: entry.name,
            workspace,
            state: entry.state,
            created: false,
        });
    }

    crate::progress::step(&msg!("progress-creating-sandbox"));
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
    verify(&entry, sandbox, &workspace)?;

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
    workspace_root: &Path,
) -> Result<()> {
    verify(entry, sandbox, &workspace_path(workspace_root, sandbox))
}

/// 既存Sandboxが期待する構成であることを確認する。
///
/// 誰が作成したかは条件にしない。案件との対応は、canonical project IDから導出した
/// 名前と、その案件だけが使う中立Workspaceの実pathで判定する。由来Templateは一覧に
/// 現れないため、同一性の根拠にしない。
fn verify(entry: &SandboxEntry, sandbox: &SandboxName, workspace: &Path) -> Result<()> {
    match &entry.workspace {
        Some(observed) => {
            let observed = paths::real_path(Path::new(observed));
            let expected = paths::real_path(workspace);
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

    Ok(())
}

/// Sandbox内にpathが存在するか。
pub fn path_exists(host: &dyn HostEnvironment, sandbox: &str, path: &str) -> Result<bool> {
    Ok(exec(host, sandbox, &["test", "-e", path])?.success())
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
        .remediation(msg!(
            "security-ssh-agent-exposed-remediation",
            command = format!("sbx rm {sandbox}")
        )),
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
#[path = "sandbox_test.rs"]
mod sandbox_test;

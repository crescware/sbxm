//! `sbxm open`。
//!
//! 登録済み案件のSandboxを起動し、SSHでterminalを引き渡す。Sandboxを新規作成しない。

use std::path::Path;
use std::time::{Duration, Instant};

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::config::{ConfigLocation, GlobalConfig};
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, ExclusiveLock, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope};
use crate::project::{ProjectId, SandboxLayout};

use super::inventory::{self, ProjectState};
use super::select::{self, ProjectPrompt};
use super::{daemon, sandbox};

/// 起動待ちのpolling設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Poll {
    pub interval: Duration,
    pub limit: Duration,
}

impl Default for Poll {
    fn default() -> Poll {
        Poll {
            interval: Duration::from_secs(2),
            limit: Duration::from_secs(60),
        }
    }
}

/// 接続先と、接続前に見せる情報。
#[derive(Debug)]
pub struct Prepared {
    pub project: String,
    pub sandbox: String,
    /// 接続先のSSH host名。
    pub ssh_host: String,
    pub worktrees: Vec<String>,
    _lock: ExclusiveLock,
}

/// SSHへ引き渡せる状態までSandboxを整える。
///
/// 1. 対象を引数またはpromptで解決する
/// 2. project lockを取得する
/// 3. Docker Engineへの疎通を確認する
/// 4. Sandbox identityとstateを検証する
/// 5. daemonを安全に再起動する
/// 6. stoppedなら起動し、runningになるまで待つ
/// 7. managed worktreeをmetadataとGitから検証する
pub fn prepare(
    config: &GlobalConfig,
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
    poll: Poll,
) -> Result<Prepared> {
    require_docker(host)?;

    let inventory = inventory::take(config, host, workspace_root)?;
    let project = select::one(&inventory, requested, prompt)?;
    let metadata = project.metadata.clone();
    let name = project.sandbox.clone();
    let state = project.state;

    let lock = paths::acquire_exclusive_lock(
        &project.paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )?;

    require_no_rebuild(&metadata)?;
    if state == ProjectState::NotCreated {
        // `open`はSandboxを新規作成しない。
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::SandboxNotCreated,
                msg!(
                    "error-sandbox-not-created",
                    project = metadata.display_id(),
                    sandbox = name
                ),
            )
            .remediation(msg!(
                "remediation-sandbox-not-created",
                command = format!("sbxm add {}", metadata.display_id())
            )),
        ));
    }

    // 接続のたびに、SSH Agentを渡さないdaemonへ入れ替える。
    let daemon_guard = daemon::restart_without_ssh_agent(host, location)?;
    if state == ProjectState::Stopped {
        start(host, name.as_str())?;
    }
    wait_until_running(host, name.as_str(), poll)?;
    drop(daemon_guard);

    let layout = SandboxLayout::new(&metadata.canonical_id);
    let worktrees = verify_worktrees(host, name.as_str(), &layout, &metadata)?;

    Ok(Prepared {
        project: metadata.display_id(),
        sandbox: name.as_str().to_string(),
        ssh_host: format!("{name}.sbx"),
        worktrees,
        _lock: lock,
    })
}

/// terminalをSSHへ引き渡す。
///
/// SSHのexit statusが0なら成功とし、非ゼロは理由を推測せず外部command失敗とする。
pub fn connect(host: &dyn HostEnvironment, prepared: &Prepared) -> Result<()> {
    let spec = CommandSpec::inherit("ssh", &[&prepared.ssh_host]);
    host.run(&spec)?.require_success()?;
    Ok(())
}

/// Docker Engineへ疎通できることを確認する。
fn require_docker(host: &dyn HostEnvironment) -> Result<()> {
    let spec = CommandSpec::probe("docker", &["version", "--format", "{{.Server.Version}}"]);
    let outcome = host.run(&spec)?;
    if outcome.success() && !outcome.stdout_text().trim().is_empty() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::DockerUnreachable,
            msg!(
                "error-docker-unreachable",
                detail = "the server version could not be read"
            ),
        )
        .remediation(msg!("remediation-start-docker")),
    ))
}

fn require_no_rebuild(metadata: &ProjectMetadata) -> Result<()> {
    if metadata.rebuild.is_none() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RebuildIntentPending,
            msg!(
                "error-rebuild-intent-pending",
                project = metadata.display_id()
            ),
        )
        .remediation(msg!(
            "remediation-run-rebuild",
            command = format!("sbxm rebuild {}", metadata.display_id())
        )),
    ))
}

/// 非対話でSandboxを起動する。
fn start(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let spec = CommandSpec::passthrough("sbx", &["exec", sandbox, "--", "/bin/true"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;
    Ok(())
}

/// runningになるまで待つ。状態は毎回structured outputから読む。
fn wait_until_running(host: &dyn HostEnvironment, name: &str, poll: Poll) -> Result<()> {
    let deadline = Instant::now() + poll.limit;
    loop {
        let entries = daemon::list(host)?;
        let entry = entries.iter().find(|entry| entry.name == name);
        match entry.map(|entry| entry.state) {
            Some(crate::compatibility::SandboxState::Running) => return Ok(()),
            _ if Instant::now() >= deadline => {
                return Err(Error::single(
                    Diagnostic::new(
                        ErrorId::SandboxNotRunning,
                        msg!(
                            "error-sandbox-not-running",
                            sandbox = name,
                            observed = entry
                                .map(|entry| match entry.state {
                                    crate::compatibility::SandboxState::Running => "running",
                                    crate::compatibility::SandboxState::Stopped => "stopped",
                                })
                                .unwrap_or("not-created")
                        ),
                    )
                    .remediation(msg!(
                        "remediation-sandbox-not-running",
                        command = format!("sbxm status {name}")
                    )),
                ));
            }
            _ => std::thread::sleep(poll.interval),
        }
    }
}

/// metadataが宣言するmanaged worktreeが、Sandbox内に揃っていることを確認する。
fn verify_worktrees(
    host: &dyn HostEnvironment,
    name: &str,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
) -> Result<Vec<String>> {
    let mut present = Vec::with_capacity(metadata.managed_worktrees.len());
    for worktree in &metadata.managed_worktrees {
        let path = format!("{}/{}", layout.bare_root(), worktree.path);
        if !sandbox::exec(host, name, &["test", "-d", &path])?.success() {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::SandboxRepositoryUnusable,
                    msg!(
                        "error-sandbox-repository-unusable",
                        path = path,
                        detail = "the metadata declares this managed worktree, but it is absent"
                    ),
                )
                .remediation(msg!("remediation-sandbox-repository-unusable", path = path)),
            ));
        }
        present.push(path);
    }
    Ok(present)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigLocation;
    use crate::metadata::{self, RebuildIntent};
    use crate::workflow::inventory::tests::{FakeSbx, Fixture, fixture};
    use crate::workflow::select::tests::ScriptedPrompt;

    fn poll() -> Poll {
        Poll {
            interval: Duration::from_millis(1),
            limit: Duration::from_millis(20),
        }
    }

    fn location(fixture: &Fixture) -> ConfigLocation {
        let home = fixture.workspace_root.parent().expect("a home directory");
        ConfigLocation::from_home(home.to_path_buf())
    }

    /// Docker疎通に応答するhost。
    fn docker_ready(host: FakeSbx) -> FakeSbx {
        host.answering("version --format {{.Server.Version}}", 0, "27.0.3\n")
    }

    #[test]
    fn a_running_project_is_opened_after_the_daemon_is_restarted() {
        let fixture = fixture();
        let project = fixture.register("Example-Org/Example-Repo");
        let running = format!("[{}]", fixture.entry(&project, "running"));
        let host = docker_ready(FakeSbx::listing(&running));

        let prepared = prepare(
            &fixture.config,
            &location(&fixture),
            None,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect("prepare");

        assert_eq!(prepared.ssh_host, format!("{}.sbx", project.sandbox));
        assert!(host.ran("daemon stop"));
        assert!(host.ran("daemon start --detach"));
        assert!(
            !host.ran("/bin/true"),
            "a running sandbox is not started again: {:?}",
            host.calls()
        );
    }

    #[test]
    fn a_stopped_project_is_started_without_a_terminal_and_waited_for() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let stopped = format!("[{}]", fixture.entry(&project, "stopped"));
        let running = format!("[{}]", fixture.entry(&project, "running"));
        let host = docker_ready(FakeSbx::listings(&[&stopped, &stopped, &running]));

        prepare(
            &fixture.config,
            &location(&fixture),
            None,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect("prepare");

        assert!(host.ran("/bin/true"), "{:?}", host.calls());
    }

    #[test]
    fn a_project_without_a_sandbox_is_sent_back_to_add() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");
        let host = docker_ready(FakeSbx::listing("[]"));

        let error = prepare(
            &fixture.config,
            &location(&fixture),
            None,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("open never creates a sandbox");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
        assert!(!host.ran("daemon stop"), "the daemon is left alone");
    }

    #[test]
    fn a_rebuild_in_progress_stops_the_connection() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let mut metadata = project.metadata.clone();
        metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: "2".repeat(64),
            previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
        });
        metadata::update(&project.paths, &metadata).expect("record the intent");

        let running = format!("[{}]", fixture.entry(&project, "running"));
        let host = docker_ready(FakeSbx::listing(&running));

        let error = prepare(
            &fixture.config,
            &location(&fixture),
            None,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("a half-switched sandbox is not opened");
        assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
        assert!(!host.ran("daemon stop"));
    }

    #[test]
    fn a_sandbox_that_never_reaches_running_is_reported_rather_than_assumed() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let stopped = format!("[{}]", fixture.entry(&project, "stopped"));
        let host = docker_ready(FakeSbx::listing(&stopped));

        let error = prepare(
            &fixture.config,
            &location(&fixture),
            None,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("a sandbox that stays stopped is not connected to");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    }

    #[test]
    fn an_engine_that_does_not_answer_stops_before_anything_is_read() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");
        let host = FakeSbx::listing("[]").answering("version --format {{.Server.Version}}", 1, "");

        let error = prepare(
            &fixture.config,
            &location(&fixture),
            None,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("without the engine there is nothing to open");
        assert_eq!(error.first_id(), Some(ErrorId::DockerUnreachable));
        assert!(!host.ran("ls --json"));
    }

    #[test]
    fn the_connection_hands_the_terminal_to_ssh() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let running = format!("[{}]", fixture.entry(&project, "running"));
        let host = docker_ready(FakeSbx::listing(&running));
        let prepared = prepare(
            &fixture.config,
            &location(&fixture),
            None,
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect("prepare");

        connect(&host, &prepared).expect("connect");
        let calls = host.calls();
        let ssh = calls.last().expect("ssh runs last");
        assert_eq!(ssh, &vec![format!("{}.sbx", project.sandbox)]);
    }
}

//! Docker Sandboxes daemonの安全な再起動。
//!
//! Sandboxを作成または再作成する前に、hostのSSH Agentがdaemon経由でSandboxへ
//! 転送されない状態を作る。daemonの安全性を永続markerやinstance IDから推測せず、
//! 毎回、全Sandboxのactive session不在を確認してから停止・起動する。

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::{SandboxEntry, parse_sandbox_list};
use crate::config::ConfigLocation;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{
    self, ExclusiveLock, LOCK_TIMEOUT, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope,
};

/// daemonを操作している区間。
///
/// この値が生きているあいだ、ほかのsbxmの実行はdaemonを操作しない。
#[derive(Debug)]
pub struct DaemonGuard {
    _lock: ExclusiveLock,
}

/// SSH Agentを渡さないdaemonへ入れ替える。
///
/// project lockを取得した後に呼ぶ。session不在を証明できない限りdaemonを変更しない。
pub fn restart_without_ssh_agent(
    host: &dyn HostEnvironment,
    location: &ConfigLocation,
) -> Result<DaemonGuard> {
    let runtime = location.runtime_dir();
    paths::ensure_private_dir(&runtime, PRIVATE_DIR_MODE, PathScope::ConfigDir)?;
    let lock = paths::acquire_exclusive_lock(
        &location.daemon_lock(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )?;

    require_no_active_session(host)?;

    // 停止と起動のあいだにSandboxを触らないよう、lockを保持したまま続ける。
    let stop = CommandSpec::capture("sbx", &["daemon", "stop"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&stop)?.require_success()?;

    let start = CommandSpec::capture("sbx", &["daemon", "start", "--detach"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&start)?.require_success()?;

    Ok(DaemonGuard { _lock: lock })
}

/// 全Sandboxにactive sessionがないことを、structured outputから確認する。
///
/// Sandboxが1件もない場合、接続中のsessionも存在しないため不在を確認できたものとする。
///
/// daemon lockの外から呼ぶ場合、結果は再起動を保証しない。破壊的な工程へ進む前に、
/// 確実に失敗する構成を先に見つけるために使う。
pub fn require_no_active_session(host: &dyn HostEnvironment) -> Result<()> {
    let sandboxes = list(host)?;

    let mut active: Vec<String> = Vec::new();
    for sandbox in &sandboxes {
        match sandbox.active_sessions {
            Some(0) => {}
            Some(count) => active.push(format!("{} ({count})", sandbox.name)),
            None => {
                // 検査機能がないruntimeでは、不在を証明できない。
                return Err(Error::single(
                    Diagnostic::new(
                        ErrorId::DaemonSessionUnobservable,
                        msg!("error-daemon-session-unobservable", sandbox = sandbox.name),
                    )
                    .remediation(msg!("remediation-daemon-session-unobservable")),
                ));
            }
        }
    }

    if !active.is_empty() {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::DaemonSessionActive,
                msg!("error-daemon-session-active", sandboxes = active.join(", ")),
            )
            .remediation(msg!("remediation-daemon-session-active")),
        ));
    }
    Ok(())
}

/// 現在のSandbox一覧。
pub fn list(host: &dyn HostEnvironment) -> Result<Vec<SandboxEntry>> {
    let spec = CommandSpec::capture("sbx", &["ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    parse_sandbox_list(&outcome.stdout_text())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandOutcome;
    use std::cell::RefCell;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;

    struct FakeSbx {
        listing: String,
        listing_fails: bool,
        calls: RefCell<Vec<CommandSpec>>,
    }

    impl FakeSbx {
        fn listing(output: &str) -> FakeSbx {
            FakeSbx {
                listing: output.to_string(),
                listing_fails: false,
                calls: RefCell::new(Vec::new()),
            }
        }

        fn failing_listing() -> FakeSbx {
            FakeSbx {
                listing: String::new(),
                listing_fails: true,
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
            let listing = spec.args.first().is_some_and(|arg| arg == "ls");
            let code = i32::from(listing && self.listing_fails);
            let stdout = if listing {
                self.listing.clone()
            } else {
                String::new()
            };
            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(code << 8),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

    fn location() -> (tempfile::TempDir, ConfigLocation) {
        let dir = tempfile::tempdir().expect("temporary home");
        let location = ConfigLocation::from_home(dir.path().to_path_buf());
        (dir, location)
    }

    fn idle(name: &str) -> String {
        format!(r#"[{{"name":"{name}","state":"running","active_sessions":0}}]"#)
    }

    #[test]
    fn a_daemon_with_no_session_is_stopped_and_started_without_the_agent() {
        let (_dir, location) = location();
        let host = FakeSbx::listing(&idle("sbxm-example"));

        let guard = restart_without_ssh_agent(&host, &location).expect("restart");

        assert_eq!(
            host.calls(),
            vec![
                vec!["ls".to_string(), "--json".to_string()],
                vec!["daemon".to_string(), "stop".to_string()],
                vec![
                    "daemon".to_string(),
                    "start".to_string(),
                    "--detach".to_string()
                ],
            ]
        );
        for spec in host.calls.borrow().iter() {
            assert_eq!(
                spec.env,
                EnvPolicy::InheritWithoutSshAgent,
                "{:?} must not carry the host SSH agent",
                spec.args
            );
        }
        drop(guard);
    }

    #[test]
    fn the_daemon_lock_is_private_survives_the_run_and_serializes_it() {
        let (_dir, location) = location();
        let host = FakeSbx::listing("[]");
        let guard = restart_without_ssh_agent(&host, &location).expect("restart");

        let lock_path = location.daemon_lock();
        assert_eq!(
            std::fs::metadata(location.runtime_dir())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            PRIVATE_DIR_MODE
        );
        assert_eq!(
            std::fs::metadata(&lock_path).unwrap().permissions().mode() & 0o777,
            PRIVATE_FILE_MODE
        );

        let error = paths::acquire_exclusive_lock(
            &lock_path,
            std::time::Duration::from_millis(100),
            PRIVATE_FILE_MODE,
            PathScope::ConfigFile,
        )
        .expect_err("a second run waits for the first");
        assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

        drop(guard);
        assert!(lock_path.exists(), "the lock file outlives the workflow");
        paths::acquire_exclusive_lock(
            &lock_path,
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ConfigFile,
        )
        .expect("the lock is released when the guard is dropped");
    }

    #[test]
    fn an_active_session_leaves_the_daemon_alone() {
        let (_dir, location) = location();
        let host = FakeSbx::listing(
            r#"[{"name":"sbxm-busy","state":"running","active_sessions":2},{"name":"sbxm-idle","state":"stopped","active_sessions":0}]"#,
        );

        let error = restart_without_ssh_agent(&host, &location)
            .expect_err("a connected session is never dropped by sbxm");
        assert_eq!(error.first_id(), Some(ErrorId::DaemonSessionActive));
        assert_eq!(
            host.calls(),
            vec![vec!["ls".to_string(), "--json".to_string()]],
            "the daemon is not touched"
        );
    }

    #[test]
    fn a_runtime_without_session_inspection_leaves_the_daemon_alone() {
        let (_dir, location) = location();
        let host = FakeSbx::listing(r#"[{"name":"sbxm-example","state":"running"}]"#);

        let error = restart_without_ssh_agent(&host, &location)
            .expect_err("absence of sessions has to be observed, not assumed");
        assert_eq!(error.first_id(), Some(ErrorId::DaemonSessionUnobservable));
        assert_eq!(host.calls().len(), 1, "the daemon is not touched");
    }

    #[test]
    fn a_listing_that_fails_or_cannot_be_read_leaves_the_daemon_alone() {
        let (_dir, location) = location();

        let host = FakeSbx::failing_listing();
        let error = restart_without_ssh_agent(&host, &location).expect_err("a failed check");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
        assert_eq!(host.calls().len(), 1);

        let host = FakeSbx::listing(r#"[{"name":"sbxm-example","state":"pausing"}]"#);
        let error = restart_without_ssh_agent(&host, &location).expect_err("an unknown state");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(host.calls().len(), 1);
    }

    #[test]
    fn a_host_without_any_sandbox_has_no_session_to_protect() {
        let (_dir, location) = location();
        let host = FakeSbx::listing("[]");
        restart_without_ssh_agent(&host, &location)
            .expect("no sandbox means no session, which is an observation rather than a guess");
        assert_eq!(host.calls().len(), 3);
    }
}

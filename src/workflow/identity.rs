//! Sandbox内のGit identity。
//!
//! 既存の設定値が同じならそのままにし、異なる場合は別の利用者のSandboxである
//! 可能性があるため自動で上書きしない。
//!
//! `gh`のprotocol設定もここに置く。値の性質が同じで、一致しない値の扱いを同じ形で
//! 決めるためである。呼ぶかどうかは`tools`が`gh`の有無で決める。

use crate::command::HostEnvironment;
use crate::config::GitIdentity;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::sandbox;

/// GitHubとのやり取りに使うprotocol。Sandbox内のcloneはHTTPSを使う。
const GIT_PROTOCOL: &str = "https";
const GITHUB_HOST: &str = "github.com";

/// Git identityを設定する。
///
/// `gh`はsbxm自身のworkflowが一度も呼ばない。Sandbox内のcloneもfetchも素のgitがHTTPSで
/// 行い、認証はcredential helperが担う。そのため`gh`の設定はここには含めない。
pub fn ensure(host: &dyn HostEnvironment, sandbox: &str, git: &GitIdentity) -> Result<()> {
    ensure_git_config(host, sandbox, "user.name", &git.user_name)?;
    ensure_git_config(host, sandbox, "user.email", &git.user_email)
}

fn ensure_git_config(
    host: &dyn HostEnvironment,
    sandbox: &str,
    key: &str,
    expected: &str,
) -> Result<()> {
    let outcome = sandbox::exec(host, sandbox, &["git", "config", "--global", "--get", key])?;
    if outcome.success() {
        let observed = outcome.stdout_text().trim().to_string();
        if observed == expected {
            return Ok(());
        }
        if !observed.is_empty() {
            return Err(mismatch(sandbox, key, &observed, expected));
        }
    }

    sandbox::exec(host, sandbox, &["git", "config", "--global", key, expected])?
        .require_success()?;
    Ok(())
}

/// Sandbox内の`gh`が使うprotocolがHTTPSであることを確かめる。
///
/// sbxmはこの値を読まない。remote URLは自分でHTTPSとして書く。`gh`自身の既定も
/// `https`であり、設定fileを持たないSandboxでは一致を観測して終わる。書き込みへ
/// 進むのは`gh`が答えなかった場合だけである。
///
/// 実際に効くのは、中で`ssh`へ変えられていた場合である。SandboxにSSH鍵は無いため、
/// その`gh`はGitHubへ到達できない。
pub(super) fn ensure_git_protocol(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["gh", "config", "get", "git_protocol", "--host", GITHUB_HOST],
    )?;
    if outcome.success() {
        let observed = outcome.stdout_text().trim().to_string();
        if observed == GIT_PROTOCOL {
            return Ok(());
        }
        if !observed.is_empty() {
            return Err(mismatch(
                sandbox,
                "gh git_protocol",
                &observed,
                GIT_PROTOCOL,
            ));
        }
    }

    sandbox::exec(
        host,
        sandbox,
        &[
            "gh",
            "config",
            "set",
            "git_protocol",
            GIT_PROTOCOL,
            "--host",
            GITHUB_HOST,
        ],
    )?
    .require_success()?;
    Ok(())
}

fn mismatch(sandbox: &str, key: &str, observed: &str, expected: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxIdentityMismatch,
            msg!(
                "error-sandbox-identity-mismatch",
                sandbox = sandbox,
                key = key,
                observed = observed,
                expected = expected
            ),
        )
        .remediation(msg!("remediation-sandbox-identity-mismatch")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutcome, CommandSpec};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;

    struct FakeSbx {
        /// Sandbox内で既に設定されている値。
        settings: HashMap<String, String>,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl FakeSbx {
        fn holding(settings: &[(&str, &str)]) -> FakeSbx {
            FakeSbx {
                settings: settings
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn stored(&self, key: &str) -> (i32, String) {
            match self.settings.get(key) {
                Some(value) => (0, format!("{value}\n")),
                None => (1, String::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }

        fn wrote(&self, value: &str) -> bool {
            self.calls()
                .iter()
                .any(|args| args.iter().any(|arg| arg == value))
        }
    }

    impl HostEnvironment for FakeSbx {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
            self.calls.borrow_mut().push(spec.args.clone());
            let inner: Vec<&str> = spec
                .args
                .iter()
                .skip_while(|arg| *arg != "--")
                .skip(1)
                .map(String::as_str)
                .collect();

            let (code, stdout) = match inner.as_slice() {
                ["git", "config", "--global", "--get", key] => self.stored(key),
                ["gh", "config", "get", "git_protocol", ..] => self.stored("git_protocol"),
                _ => (0, String::new()),
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

    fn identity() -> GitIdentity {
        GitIdentity {
            user_name: "Example User".into(),
            user_email: "user@example.com".into(),
        }
    }

    #[test]
    fn an_unset_sandbox_is_configured_from_the_global_configuration() {
        let host = FakeSbx::holding(&[]);
        ensure(&host, "sbxm-example", &identity()).expect("configure");

        assert!(host.wrote("Example User"));
        assert!(host.wrote("user@example.com"));
        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.iter().any(|arg| arg == "gh")),
            "gh belongs to the tool listing, not to the git identity: {:?}",
            host.calls()
        );
    }

    #[test]
    fn the_protocol_is_written_only_when_it_is_not_already_https() {
        let host = FakeSbx::holding(&[]);
        ensure_git_protocol(&host, "sbxm-example").expect("configure");
        assert!(
            host.calls()
                .iter()
                .any(|args| args.contains(&"set".to_string())
                    && args.contains(&GIT_PROTOCOL.to_string()))
        );

        // ghの既定は`https`である。一致を観測したSandboxへは書き込まない。
        let host = FakeSbx::holding(&[("git_protocol", GIT_PROTOCOL)]);
        ensure_git_protocol(&host, "sbxm-example").expect("nothing to do");
        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.contains(&"set".to_string())),
            "{:?}",
            host.calls()
        );
    }

    #[test]
    fn a_protocol_that_was_changed_by_hand_stops_the_run() {
        let host = FakeSbx::holding(&[("git_protocol", "ssh")]);
        let error =
            ensure_git_protocol(&host, "sbxm-example").expect_err("a different value is refused");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxIdentityMismatch));
    }

    #[test]
    fn values_that_already_match_are_left_as_they_are() {
        let host = FakeSbx::holding(&[
            ("user.name", "Example User"),
            ("user.email", "user@example.com"),
        ]);
        ensure(&host, "sbxm-example", &identity()).expect("nothing to do");

        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.contains(&"set".to_string())),
            "an already configured sandbox is not written to: {:?}",
            host.calls()
        );
    }

    #[test]
    fn a_sandbox_configured_for_someone_else_is_not_overwritten() {
        for settings in [
            vec![("user.name", "Another User")],
            vec![
                ("user.name", "Example User"),
                ("user.email", "another@example.com"),
            ],
        ] {
            let host = FakeSbx::holding(&settings);
            let error = ensure(&host, "sbxm-example", &identity())
                .expect_err("a different value belongs to someone else");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::SandboxIdentityMismatch),
                "{settings:?} produced the wrong error"
            );
            assert!(
                !host
                    .calls()
                    .iter()
                    .any(|args| args.contains(&"Example User".to_string())
                        && args.contains(&"--global".to_string())
                        && !args.contains(&"--get".to_string())),
                "nothing is written while a value disagrees: {:?}",
                host.calls()
            );
        }
    }
}

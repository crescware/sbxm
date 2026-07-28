//! Sandbox内のGit identityとGitHub protocol。
//!
//! 既存の設定値が同じならそのままにし、異なる場合は別の利用者のSandboxである
//! 可能性があるため自動で上書きしない。
//!
//! `gh`はsbxm自身のworkflowが使わない。Sandbox内のcloneもfetchも素のgitがHTTPSで
//! 行い、認証はcredential helperが担う。`gh`を入れていないSandboxを構築の失敗に
//! しないため、`gh`向けの設定は`gh`があるときにだけ行う。

use crate::command::HostEnvironment;
use crate::config::GitIdentity;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::sandbox;

/// GitHubとのやり取りに使うprotocol。Sandbox内のcloneはHTTPSを使う。
const GIT_PROTOCOL: &str = "https";
const GITHUB_HOST: &str = "github.com";

/// Git identityとGitHub protocolを設定する。
pub fn ensure(host: &dyn HostEnvironment, sandbox: &str, git: &GitIdentity) -> Result<()> {
    ensure_git_config(host, sandbox, "user.name", &git.user_name)?;
    ensure_git_config(host, sandbox, "user.email", &git.user_email)?;
    // `gh`はこの先の工程が一度も呼ばない。入れていないSandboxはそのまま先へ進める。
    if has_gh(host, sandbox)? {
        ensure_git_protocol(host, sandbox)?;
    }
    Ok(())
}

/// Sandbox内に`gh`があるかを、中から確かめるscript。
///
/// 見つからない場合も成功で終え、標準出力の有無で答える。exit statusで分けると、
/// 「`gh`が無い」と「検査自体が実行できなかった」を区別できない。
pub(super) fn gh_probe() -> &'static str {
    "if command -v gh > /dev/null 2>&1; then printf yes; fi"
}

/// Sandbox内に`gh`があるか。
///
/// Dockerfileは利用者の持ち物であり、記録から推定せずSandboxを観測して答える。
fn has_gh(host: &dyn HostEnvironment, sandbox: &str) -> Result<bool> {
    let outcome = sandbox::exec(host, sandbox, &["sh", "-c", gh_probe()])?.require_success()?;
    Ok(!outcome.stdout_text().trim().is_empty())
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

/// Sandbox内の`gh`が使うprotocolをHTTPSにする。
///
/// sbxmが読む値ではない。sbxmはremote URLを自分でHTTPSとして書く。SandboxにSSH鍵は
/// 無いため、`gh`の既定のままでは中で実行した`gh repo clone`がsshを引いて失敗する。
/// `gh`を入れた利用者のために、それを先回りで避ける。
fn ensure_git_protocol(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
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
        /// Sandbox内に`gh`があるか。
        gh: bool,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl FakeSbx {
        fn holding(settings: &[(&str, &str)]) -> FakeSbx {
            FakeSbx {
                settings: settings
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
                gh: true,
                calls: RefCell::new(Vec::new()),
            }
        }

        /// `gh`を入れずに組んだDockerfileから作られたSandbox。
        fn without_gh() -> FakeSbx {
            FakeSbx {
                settings: HashMap::new(),
                gh: false,
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
                ["sh", "-c", script] if *script == gh_probe() => {
                    (0, if self.gh { "yes" } else { "" }.to_string())
                }
                // `gh`の無いSandboxで`gh`を起動しようとしたら、testとして失敗させる。
                ["gh", ..] if !self.gh => {
                    panic!("a sandbox without gh must not be asked to run it: {inner:?}")
                }
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
        assert!(host.wrote("git_protocol"));
        assert!(
            host.calls()
                .iter()
                .any(|args| args.contains(&"set".to_string())
                    && args.contains(&GIT_PROTOCOL.to_string()))
        );
    }

    #[test]
    fn a_sandbox_without_gh_is_configured_all_the_same() {
        let host = FakeSbx::without_gh();
        ensure(&host, "sbxm-example", &identity()).expect("gh is not part of the workflow");

        assert!(host.wrote("Example User"));
        assert!(host.wrote("user@example.com"));
        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.iter().any(|arg| arg == "gh")),
            "a sandbox that does not carry gh is never asked to run it: {:?}",
            host.calls()
        );
    }

    #[test]
    fn values_that_already_match_are_left_as_they_are() {
        let host = FakeSbx::holding(&[
            ("user.name", "Example User"),
            ("user.email", "user@example.com"),
            ("git_protocol", "https"),
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
            vec![
                ("user.name", "Example User"),
                ("user.email", "user@example.com"),
                ("git_protocol", "ssh"),
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

//! 案件限定のGitHub secret。
//!
//! tokenの発行と入力は自動化しない。存在確認だけをread-onlyで行い、値は取得も
//! 表示もしない。

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::parse_secret_names;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// Sandbox内でGitHubへのaccessに使うsecretの名前。
pub const GITHUB_SECRET: &str = "github";

/// GitHub secretが登録済みであることを確認する。
///
/// 未登録なら、発行条件と登録commandを示して前提条件不足として停止する。登録後は
/// 同じ`add`を再実行すると、Sandboxを再利用して次の工程へ進む。
pub fn require_github(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let spec = CommandSpec::capture("sbx", &["secret", "ls", sandbox, "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    let names = parse_secret_names(&outcome.stdout_text())?;

    if names.iter().any(|name| name == GITHUB_SECRET) {
        return Ok(());
    }

    Err(Error::single(
        Diagnostic::new(
            ErrorId::GithubSecretMissing,
            msg!(
                "error-github-secret-missing",
                sandbox = sandbox,
                secret = GITHUB_SECRET
            ),
        )
        .remediation(msg!(
            "remediation-github-secret-missing",
            command = format!("sbx secret set {sandbox} {GITHUB_SECRET}")
        )),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandOutcome;
    use std::cell::RefCell;
    use std::os::unix::process::ExitStatusExt;

    struct FakeSbx {
        listing: String,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl FakeSbx {
        fn listing(output: &str) -> FakeSbx {
            FakeSbx {
                listing: output.to_string(),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl HostEnvironment for FakeSbx {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
            self.calls.borrow_mut().push(spec.args.clone());
            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(0),
                stdout: self.listing.clone().into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

    #[test]
    fn a_registered_secret_lets_the_build_continue() {
        let host = FakeSbx::listing(r#"[{"name":"github"},{"name":"other"}]"#);
        require_github(&host, "sbxm-example").expect("the secret is there");

        let calls = host.calls.borrow();
        assert_eq!(
            calls[0],
            vec![
                "secret".to_string(),
                "ls".to_string(),
                "sbxm-example".to_string(),
                "--json".to_string()
            ],
            "the check is read-only"
        );
    }

    #[test]
    fn a_missing_secret_stops_with_the_command_that_registers_it() {
        let host = FakeSbx::listing("[]");
        let error = require_github(&host, "sbxm-example")
            .expect_err("a build without repository access cannot continue");

        assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
        let remediation = error.diagnostics()[0]
            .remediation
            .as_ref()
            .expect("the user is told how to register it");
        assert_eq!(remediation.id, "remediation-github-secret-missing");
        assert!(
            remediation
                .args
                .iter()
                .any(|(_, value)| value == "sbx secret set sbxm-example github")
        );
    }

    #[test]
    fn the_value_of_a_secret_is_never_requested() {
        let host = FakeSbx::listing(r#"[{"name":"github"}]"#);
        require_github(&host, "sbxm-example").expect("the secret is there");

        for args in host.calls.borrow().iter() {
            assert!(
                !args.iter().any(|arg| arg == "get" || arg == "reveal"),
                "sbxm only asks whether the secret exists: {args:?}"
            );
        }
    }
}

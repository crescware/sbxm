//! Docker Sandboxes daemonの観測。
//!
//! sbxmはdaemonを停止も起動もしない。daemonを止めるには動作中のSandboxを止める
//! 必要があり、作業中のSandboxを巻き込むためである。
//!
//! hostのSSH AgentがSandboxへ渡っていないことは、daemonの起動条件から推定せず、
//! 作成したSandboxの中から観測する（`sandbox::require_credentials_isolated`）。

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::{SandboxEntry, parse_sandbox_list};
use crate::error::Result;

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

    #[test]
    fn the_listing_is_read_without_the_host_ssh_agent() {
        let host =
            FakeSbx::listing(r#"{"sandboxes":[{"name":"sbxm-example","status":"running"}]}"#);

        let entries = list(&host).expect("the listing parses");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "sbxm-example");

        let calls = host.calls.borrow();
        assert_eq!(calls[0].args, vec!["ls".to_string(), "--json".to_string()]);
        assert_eq!(calls[0].env, EnvPolicy::InheritWithoutSshAgent);
    }

    #[test]
    fn a_listing_that_fails_is_not_read_as_an_empty_one() {
        let host = FakeSbx::failing_listing();
        let error = list(&host).expect_err("a failed listing is not no sandboxes");
        assert_eq!(
            error.first_id(),
            Some(crate::error::ErrorId::ExternalCommandFailed)
        );
    }
}

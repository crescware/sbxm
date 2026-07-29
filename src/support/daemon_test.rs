use super::*;
use crate::command::CommandOutcome;
use std::cell::RefCell;

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
        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}

#[test]
fn the_listing_is_read_without_the_host_ssh_agent() {
    let host = FakeSbx::listing(r#"{"sandboxes":[{"name":"sbxm-example","status":"running"}]}"#);

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

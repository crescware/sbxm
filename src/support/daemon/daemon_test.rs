use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment};
use crate::diagnostics::Result;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::boundary::host::CommandOutcome;
use std::cell::RefCell;

struct FakeSbx {
    /// `sbx ls`への応答を呼び出し順に並べる。末尾から取り出し、尽きたら最後の1件を繰り返す。
    listings: RefCell<Vec<String>>,
    listing_fails: bool,
    calls: RefCell<Vec<CommandSpec>>,
}

impl FakeSbx {
    fn listing(output: &str) -> FakeSbx {
        FakeSbx {
            listings: RefCell::new(vec![output.to_string()]),
            listing_fails: false,
            calls: RefCell::new(Vec::new()),
        }
    }

    /// 呼び出しごとに異なる一覧を返す。daemonが暖まるまでの振る舞いを再現する。
    fn sequenced_listing(outputs: &[&str]) -> FakeSbx {
        FakeSbx {
            listings: RefCell::new(
                outputs
                    .iter()
                    .rev()
                    .map(|value| (*value).to_string())
                    .collect(),
            ),
            listing_fails: false,
            calls: RefCell::new(Vec::new()),
        }
    }

    fn failing_listing() -> FakeSbx {
        FakeSbx {
            listings: RefCell::new(vec![String::new()]),
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
            let mut listings = self.listings.borrow_mut();
            if listings.len() > 1 {
                listings.pop().unwrap_or_default()
            } else {
                listings.last().cloned().unwrap_or_default()
            }
        } else {
            String::new()
        };
        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}

#[test]
fn the_listing_is_read_without_the_host_ssh_agent() -> Checked {
    let host = FakeSbx::listing(r#"{"sandboxes":[{"name":"sbxm-example","status":"running"}]}"#);

    let entries = list(&host).required_because("the listing parses")?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "sbxm-example");

    let calls = host.calls.borrow();
    assert_eq!(calls[0].args, vec!["ls".to_string(), "--json".to_string()]);
    assert_eq!(calls[0].env, EnvPolicy::InheritWithoutSshAgent);
    Ok(())
}

#[test]
fn a_listing_that_fails_is_not_read_as_an_empty_one() -> Checked {
    let host = FakeSbx::failing_listing();
    let error = list(&host).refused_because("a failed listing is not no sandboxes")?;
    assert_eq!(
        error.first_id(),
        Some(crate::diagnostics::ErrorId::ExternalCommandFailed)
    );
    Ok(())
}

#[test]
fn a_listing_cut_short_by_a_cold_daemon_is_retried_until_it_parses() -> Checked {
    let host = FakeSbx::sequenced_listing(&[
        "{",
        r#"{"sandboxes":[{"name":"sbxm-example","status":"running"}]}"#,
    ]);

    let entries = list(&host).required_because("the retry reads the second, complete listing")?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "sbxm-example");
    assert_eq!(
        host.calls.borrow().len(),
        2,
        "the command is run again after the unparseable listing"
    );
    Ok(())
}

#[test]
fn a_listing_that_stays_unparseable_gives_up_after_a_bounded_number_of_retries() -> Checked {
    let host = FakeSbx::sequenced_listing(&["{"]);

    let error = list(&host).refused_because("retries do not run forever")?;
    assert_eq!(
        error.first_id(),
        Some(crate::diagnostics::ErrorId::ExternalOutputUnparseable)
    );
    assert_eq!(
        host.calls.borrow().len(),
        3,
        "the initial attempt plus every retry ran, then it stopped"
    );
    Ok(())
}

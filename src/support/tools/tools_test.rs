use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use std::fmt::Write as _;

use crate::testing::outcome::{Checked, Required};

use super::*;
use crate::boundary::host::{CommandOutcome, CommandSpec};
use std::cell::RefCell;

/// probeへ決め打ちで答え、それ以外の起動を成功として扱うhost。
struct FakeSbx {
    named: String,
    calls: RefCell<Vec<Vec<String>>>,
}

impl FakeSbx {
    fn naming(tools: &[&str]) -> FakeSbx {
        FakeSbx {
            named: tools.iter().fold(String::new(), |mut out, name| {
                // Stringへの書き込みは失敗しない。
                let _ = writeln!(out, "{name}");
                out
            }),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }

    fn ran(&self, needle: &str) -> bool {
        self.calls()
            .iter()
            .any(|args| args.iter().any(|arg| arg.contains(needle)))
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.args.clone());
        let inner = crate::testing::command::inner_args(spec);

        let (code, stdout) = match inner.as_slice() {
            ["sh", "-c", script] if *script == probe() => (0, self.named.clone()),
            _ => (0, String::new()),
        };

        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}

#[test]
fn the_probe_asks_for_every_tool_the_listing_holds() {
    let script = probe();
    for tool in ALL {
        assert!(
            script.contains(tool.name()),
            "the probe has to ask for {}",
            tool.name()
        );
    }
}

#[test]
fn one_run_answers_for_every_tool() -> Checked {
    let host = FakeSbx::naming(&["gh", "mise", "claude", "codex"]);
    let installed =
        Installed::observe(&host, "sbxm-example").required_because("count the tools")?;
    for tool in ALL {
        assert!(installed.has(tool), "{} was named", tool.name());
    }
    assert_eq!(
        host.calls().len(),
        1,
        "the whole listing is answered by one run: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_tool_that_was_not_named_is_absent() -> Checked {
    let installed = Installed::observe(&FakeSbx::naming(&["gh", "codex"]), "sbxm-example")
        .required_because("count")?;
    assert!(installed.has(&Gh));
    assert!(installed.has(&Codex));
    assert!(!installed.has(&Mise));
    assert!(!installed.has(&Claude));
    Ok(())
}

#[test]
fn a_name_the_listing_does_not_hold_is_ignored() -> Checked {
    let installed = Installed::observe(&FakeSbx::naming(&["gh", "brew"]), "sbxm-example")
        .required_because("count")?;
    assert!(installed.has(&Gh));
    assert_eq!(installed.0.len(), 1, "{installed:?}");
    Ok(())
}

#[test]
fn a_sandbox_that_names_nothing_carries_nothing() -> Checked {
    let installed =
        Installed::observe(&FakeSbx::naming(&[]), "sbxm-example").required_because("count")?;
    assert_eq!(installed, Installed::default());
    Ok(())
}

#[test]
fn only_the_tools_that_are_there_are_told_what_happened() -> Checked {
    // ghが無いSandboxは、ghの設定を一度も試されない。
    let host = FakeSbx::naming(&["mise", "claude", "codex"]);
    SandboxReady::announce(&host, "sbxm-example").required_because("nothing to configure")?;
    assert!(
        !host.ran("git_protocol"),
        "a sandbox without gh is never asked to configure it: {:?}",
        host.calls()
    );

    let host = FakeSbx::naming(&["gh"]);
    SandboxReady::announce(&host, "sbxm-example").required_because("configure gh")?;
    assert!(host.ran("git_protocol"), "{:?}", host.calls());
    Ok(())
}

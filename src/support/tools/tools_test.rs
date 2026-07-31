use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::SandboxLayout;

use std::fmt::Write as _;

use crate::testing::outcome::{Checked, Required};

use super::*;
use crate::command::{CommandOutcome, CommandSpec};
use std::cell::RefCell;

/// probeへ決め打ちで答え、それ以外の起動を成功として扱うhost。
struct FakeSbx {
    named: String,
    present: Vec<String>,
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
            present: Vec::new(),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// Sandbox内に存在するfile。
    fn holding(mut self, paths: &[&str]) -> FakeSbx {
        self.present = paths.iter().map(|path| (*path).to_string()).collect();
        self
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
            ["test", "-f", path] => {
                if self.present.iter().any(|known| known == path) {
                    (0, String::new())
                } else {
                    (1, String::new())
                }
            }
            _ => (0, String::new()),
        };

        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}

fn layout() -> Checked<SandboxLayout> {
    Ok(SandboxLayout::new(
        &crate::project::ProjectId::parse("example-org/example-repo")
            .required_because("valid project id")?
            .canonical(),
    ))
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

#[test]
fn mise_names_the_worktrees_that_declare_it() -> Checked {
    let declared = "/home/agent/work/example-repo/example-repo.tree-0/mise.toml";
    let host = FakeSbx::naming(&["gh", "mise", "claude", "codex"]).holding(&[declared]);

    let notes = WorktreesReady::announce(&host, "sbxm-example", &layout()?, 1)
        .required_because("raise the event")?;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].items, vec![declared.to_string()]);
    assert_eq!(notes[0].heading.id, "add-mise-heading");
    assert_eq!(notes[0].hint.id, "add-mise-hint");
    Ok(())
}

#[test]
fn a_sandbox_without_mise_is_never_told_to_run_mise() -> Checked {
    let declared = "/home/agent/work/example-repo/example-repo.tree-0/mise.toml";
    let host = FakeSbx::naming(&["gh", "claude", "codex"]).holding(&[declared]);

    let notes = WorktreesReady::announce(&host, "sbxm-example", &layout()?, 1)
        .required_because("raise the event")?;
    assert!(
        notes.is_empty(),
        "the hint tells the user to run mise, which this sandbox does not carry: {notes:?}"
    );
    assert!(
        !host.ran("mise.toml"),
        "the declared files are not even looked for: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_worktree_without_a_declaration_produces_no_note() -> Checked {
    let host = FakeSbx::naming(&["mise"]);
    let notes = WorktreesReady::announce(&host, "sbxm-example", &layout()?, 1)
        .required_because("raise the event")?;
    assert!(notes.is_empty(), "{notes:?}");
    Ok(())
}

use super::*;
use crate::command::{CommandOutcome, CommandSpec};
use std::cell::RefCell;
use std::os::unix::process::ExitStatusExt;

/// probeへ決め打ちで答え、それ以外の起動を成功として扱うhost。
struct FakeSbx {
    named: String,
    present: Vec<String>,
    calls: RefCell<Vec<Vec<String>>>,
}

impl FakeSbx {
    fn naming(tools: &[&str]) -> FakeSbx {
        FakeSbx {
            named: tools
                .iter()
                .map(|name| format!("{name}\n"))
                .collect::<String>(),
            present: Vec::new(),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// Sandbox内に存在するfile。
    fn holding(mut self, paths: &[&str]) -> FakeSbx {
        self.present = paths.iter().map(|path| path.to_string()).collect();
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
        let inner: Vec<&str> = spec
            .args
            .iter()
            .skip_while(|arg| *arg != "--")
            .skip(1)
            .map(String::as_str)
            .collect();

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

fn layout() -> SandboxLayout {
    SandboxLayout::new(
        &crate::project::ProjectId::parse("example-org/example-repo")
            .expect("valid project id")
            .canonical(),
    )
}

#[test]
fn the_probe_asks_for_every_tool_the_listing_holds() {
    let script = probe();
    for tool in TOOLS {
        assert!(
            script.contains(tool.name()),
            "the probe has to ask for {}",
            tool.name()
        );
    }
}

#[test]
fn one_run_answers_for_every_tool() {
    let host = FakeSbx::naming(&["gh", "mise", "claude", "codex"]);
    let installed = installed(&host, "sbxm-example").expect("count the tools");
    for tool in TOOLS {
        assert!(installed.has(tool), "{} was named", tool.name());
    }
    assert_eq!(
        host.calls().len(),
        1,
        "the whole listing is answered by one run: {:?}",
        host.calls()
    );
}

#[test]
fn a_tool_that_was_not_named_is_absent() {
    let installed = installed(&FakeSbx::naming(&["gh", "codex"]), "sbxm-example").expect("count");
    assert!(installed.has(&Gh));
    assert!(installed.has(&Codex));
    assert!(!installed.has(&Mise));
    assert!(!installed.has(&Claude));
}

#[test]
fn a_name_the_listing_does_not_hold_is_ignored() {
    let installed = installed(&FakeSbx::naming(&["gh", "brew"]), "sbxm-example").expect("count");
    assert!(installed.has(&Gh));
    assert_eq!(installed.0.len(), 1, "{installed:?}");
}

#[test]
fn a_sandbox_that_names_nothing_carries_nothing() {
    let installed = installed(&FakeSbx::naming(&[]), "sbxm-example").expect("count");
    assert_eq!(installed, Installed::default());
}

#[test]
fn only_the_tools_that_are_there_are_told_what_happened() {
    // ghが無いSandboxは、ghの設定を一度も試されない。
    let host = FakeSbx::naming(&["mise", "claude", "codex"]);
    sandbox_ready(&host, "sbxm-example").expect("nothing to configure");
    assert!(
        !host.ran("git_protocol"),
        "a sandbox without gh is never asked to configure it: {:?}",
        host.calls()
    );

    let host = FakeSbx::naming(&["gh"]);
    sandbox_ready(&host, "sbxm-example").expect("configure gh");
    assert!(host.ran("git_protocol"), "{:?}", host.calls());
}

#[test]
fn mise_names_the_worktrees_that_declare_it() {
    let declared = "/home/agent/work/example-repo/example-repo.tree-0/mise.toml";
    let host = FakeSbx::naming(&["gh", "mise", "claude", "codex"]).holding(&[declared]);

    let notes = worktrees_ready(&host, "sbxm-example", &layout(), 1).expect("raise the event");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].items, vec![declared.to_string()]);
    assert_eq!(notes[0].heading.id, "add-mise-heading");
    assert_eq!(notes[0].hint.id, "add-mise-hint");
}

#[test]
fn a_sandbox_without_mise_is_never_told_to_run_mise() {
    let declared = "/home/agent/work/example-repo/example-repo.tree-0/mise.toml";
    let host = FakeSbx::naming(&["gh", "claude", "codex"]).holding(&[declared]);

    let notes = worktrees_ready(&host, "sbxm-example", &layout(), 1).expect("raise the event");
    assert!(
        notes.is_empty(),
        "the hint tells the user to run mise, which this sandbox does not carry: {notes:?}"
    );
    assert!(
        !host.ran("mise.toml"),
        "the declared files are not even looked for: {:?}",
        host.calls()
    );
}

#[test]
fn a_worktree_without_a_declaration_produces_no_note() {
    let host = FakeSbx::naming(&["mise"]);
    let notes = worktrees_ready(&host, "sbxm-example", &layout(), 1).expect("raise the event");
    assert!(notes.is_empty(), "{notes:?}");
}

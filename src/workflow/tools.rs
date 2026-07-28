//! 既定のtemplateが入れる、sbxm自身の動作には要らないtool。
//!
//! Dockerfileは利用者の持ち物であり、この4つはどれも削れる。何が入っているかは
//! metadataに残らないため、Sandboxを観測して決める。
//!
//! toolは「何が起きたら自分は何をするか」だけを宣言する。何もしないtoolは既定の
//! noopのままにする。eventを上げる側はtoolを名指しせず、`TOOLS`を順に回す。
//!
//! eventはcommandではなく、起きたことで切る。`prepare`と`rebuild`はどちらもSandboxを
//! 使える状態にするため、同じeventを上げる。

use std::collections::BTreeSet;

use crate::command::HostEnvironment;
use crate::error::{Msg, Result};
use crate::msg;
use crate::project::SandboxLayout;

use super::identity;
use super::sandbox;

/// `mise`の設定を持つと判断するfile。
const MISE_FILES: [&str; 3] = ["mise.toml", ".mise.toml", ".tool-versions"];

/// toolが利用者へ返す案内。
///
/// sbxmが代わりに実行しないことを示すために使う。errorではないため、stdoutへ出す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub heading: Msg,
    /// 案内の対象。pathや識別子であり、翻訳しない。
    pub items: Vec<String>,
    pub hint: Msg,
}

/// Sandboxが使える状態になった瞬間。
pub struct SandboxReady<'a> {
    pub host: &'a dyn HostEnvironment,
    pub sandbox: &'a str,
}

/// managed worktreeが揃った瞬間。
pub struct WorktreesReady<'a> {
    pub host: &'a dyn HostEnvironment,
    pub sandbox: &'a str,
    pub layout: &'a SandboxLayout,
    pub count: usize,
    pub notes: &'a mut Vec<Note>,
}

/// sbxmが一度も起動しないtool。
pub trait Tool {
    /// Sandbox内でのcommand名。Dockerfileのmarker名にも使う。
    fn name(&self) -> &'static str;

    /// Sandboxが使える状態になったとき。`prepare`と`rebuild`が上げる。
    fn on_sandbox_ready(&self, ready: &mut SandboxReady) -> Result<()> {
        let _ = ready;
        Ok(())
    }

    /// managed worktreeが揃ったとき。`prepare`と`apply --worktrees`が上げる。
    fn on_worktrees_ready(&self, ready: &mut WorktreesReady) -> Result<()> {
        let _ = ready;
        Ok(())
    }
}

/// 並びはこの1箇所だけが持つ。probeも、checkboxも、Dockerfileのmarkerもここから引く。
pub const TOOLS: [&dyn Tool; 4] = [&Gh, &Mise, &Claude, &Codex];

/// GitHub CLI。
pub struct Gh;

impl Tool for Gh {
    fn name(&self) -> &'static str {
        "gh"
    }

    fn on_sandbox_ready(&self, ready: &mut SandboxReady) -> Result<()> {
        identity::ensure_git_protocol(ready.host, ready.sandbox)
    }
}

/// toolchain manager。
pub struct Mise;

impl Tool for Mise {
    fn name(&self) -> &'static str {
        "mise"
    }

    fn on_worktrees_ready(&self, ready: &mut WorktreesReady) -> Result<()> {
        let mut items = Vec::new();
        for index in 0..ready.count as u32 {
            let path = ready.layout.worktree(index);
            for name in MISE_FILES {
                let target = format!("{path}/{name}");
                if sandbox::exec(ready.host, ready.sandbox, &["test", "-f", &target])?.success() {
                    items.push(target);
                }
            }
        }
        if !items.is_empty() {
            // sbxmはmiseを自動実行しない。案内だけを行う。
            ready.notes.push(Note {
                heading: msg!("add-mise-heading"),
                items,
                hint: msg!("add-mise-hint"),
            });
        }
        Ok(())
    }
}

/// Claude Code。sbxmは何も行わない。
pub struct Claude;

impl Tool for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }
}

/// Codex CLI。sbxmは何も行わない。
pub struct Codex;

impl Tool for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }
}

/// Sandboxが持っているtool。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed(BTreeSet<&'static str>);

impl Installed {
    pub fn has(&self, tool: &dyn Tool) -> bool {
        self.0.contains(tool.name())
    }
}

/// Sandbox内のtoolを一度に数えるscript。
///
/// 1つも無い場合も成功で終え、標準出力へ並ぶ名前で答える。exit statusで分けると、
/// 「toolが無い」と「検査自体が実行できなかった」を区別できない。
pub fn probe() -> String {
    let names: Vec<&str> = TOOLS.iter().map(|tool| tool.name()).collect();
    format!(
        "for c in {}; do command -v \"$c\" > /dev/null 2>&1 && printf '%s\\n' \"$c\"; done",
        names.join(" ")
    )
}

/// Sandboxが持っているtoolを、1回の起動で数える。
pub fn installed(host: &dyn HostEnvironment, sandbox: &str) -> Result<Installed> {
    let outcome = sandbox::exec(host, sandbox, &["sh", "-c", &probe()])?.require_success()?;
    let answer = outcome.stdout_text();
    let named: Vec<&str> = answer.lines().map(str::trim).collect();
    Ok(Installed(
        TOOLS
            .iter()
            .map(|tool| tool.name())
            .filter(|name| named.contains(name))
            .collect(),
    ))
}

/// Sandboxが使える状態になったことを、入っているtoolへ伝える。
pub fn sandbox_ready(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let installed = installed(host, sandbox)?;
    let mut ready = SandboxReady { host, sandbox };
    for tool in TOOLS {
        if installed.has(tool) {
            tool.on_sandbox_ready(&mut ready)?;
        }
    }
    Ok(())
}

/// managed worktreeが揃ったことを、入っているtoolへ伝える。
pub fn worktrees_ready(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    count: usize,
) -> Result<Vec<Note>> {
    let installed = installed(host, sandbox)?;
    let mut notes = Vec::new();
    let mut ready = WorktreesReady {
        host,
        sandbox,
        layout,
        count,
        notes: &mut notes,
    };
    for tool in TOOLS {
        if installed.has(tool) {
            tool.on_worktrees_ready(&mut ready)?;
        }
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
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
        let installed =
            installed(&FakeSbx::naming(&["gh", "codex"]), "sbxm-example").expect("count");
        assert!(installed.has(&Gh));
        assert!(installed.has(&Codex));
        assert!(!installed.has(&Mise));
        assert!(!installed.has(&Claude));
    }

    #[test]
    fn a_name_the_listing_does_not_hold_is_ignored() {
        let installed =
            installed(&FakeSbx::naming(&["gh", "brew"]), "sbxm-example").expect("count");
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
}

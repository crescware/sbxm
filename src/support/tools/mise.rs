use crate::diagnostics::Result;
use crate::msg;

use crate::design::CommandLine;

use crate::support::sandbox;

use super::{Note, Tool, WorktreesReady};

/// `mise`の設定を持つと判断するfile。
const MISE_FILES: [&str; 3] = ["mise.toml", ".mise.toml", ".tool-versions"];

/// 利用者がSandboxの中で自分で実行するcommand。sbxmは代わりに実行しない。
const MISE_COMMANDS: [&str; 2] = ["mise trust", "mise install"];

/// toolchain manager。
pub struct Mise;

impl Tool for Mise {
    fn name(&self) -> &'static str {
        "mise"
    }

    fn on_worktrees_ready(&self, ready: &mut WorktreesReady) -> Result<()> {
        let mut items = Vec::new();
        // managed worktreeの個数は設定の上限で抑えられており、u32へ収まる。
        let count = u32::try_from(ready.count).unwrap_or(u32::MAX);
        for index in 0..count {
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
                commands: MISE_COMMANDS
                    .iter()
                    .filter_map(|command| CommandLine::optional(*command))
                    .collect(),
            });
        }
        Ok(())
    }
}

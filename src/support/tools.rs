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

use crate::ui::CommandLine;

use super::identity;
use super::sandbox;

/// `mise`の設定を持つと判断するfile。
const MISE_FILES: [&str; 3] = ["mise.toml", ".mise.toml", ".tool-versions"];

/// 利用者がSandboxの中で自分で実行するcommand。sbxmは代わりに実行しない。
const MISE_COMMANDS: [&str; 2] = ["mise trust", "mise install"];

/// toolが利用者へ返す案内。
///
/// sbxmが代わりに実行しないことを示すために使う。errorではないため、stdoutへ出す。
///
/// 実行を求めるcommandは説明文へ埋め込まず、typedな一行として持つ。rendererが独立した
/// blockとして描き、利用者はそのまま複写できる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub heading: Msg,
    /// 案内の対象。pathや識別子であり、翻訳しない。
    pub items: Vec<String>,
    pub hint: Msg,
    /// 利用者が自分で実行するcommand。
    pub commands: Vec<CommandLine>,
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

/// sbxmが利用者の作業の代わりに起動することのないtool。設定と観測だけを行う。
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
#[path = "tools_test.rs"]
mod tools_test;

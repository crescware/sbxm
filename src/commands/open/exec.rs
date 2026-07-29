//! `open`の実行と出力。

use std::io::Write;

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::msg;
use crate::project::ProjectId;
use crate::support::display::{format_or_report, text_or_report};
use crate::support::select::TerminalProjectPrompt;
use crate::support::{inventory, sandbox};

use super::super::{Context, report};

pub fn exec(project: Option<&ProjectId>, context: &Context) -> ExitCode {
    let (config, catalog) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    let mut prompt = TerminalProjectPrompt {
        heading: "select-open-heading",
        locale: catalog.locale(),
    };
    let prepared = match super::run::prepare(
        &config,
        project,
        &RealHost,
        &mut prompt,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        inventory::Poll::default(),
    ) {
        Ok(prepared) => prepared,
        Err(error) => return report(&catalog, &error),
    };

    // 接続先はterminalを引き渡す前に見せる。
    let mut stderr = std::io::stderr();
    let _ = writeln!(
        stderr,
        "{}",
        format_or_report(
            &catalog,
            &msg!(
                "open-connecting",
                project = prepared.project,
                sandbox = prepared.sandbox
            )
        )
    );
    if !prepared.worktrees.is_empty() {
        let _ = writeln!(stderr, "{}", text_or_report(&catalog, "open-worktrees"));
        for worktree in &prepared.worktrees {
            let _ = writeln!(stderr, "  {worktree}");
        }
    }

    match super::run::connect(&RealHost, &prepared) {
        Ok(()) => ExitCode::Success,
        Err(error) => report(&catalog, &error),
    }
}

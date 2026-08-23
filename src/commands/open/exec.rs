//! `open`の実行と出力。

use crate::boundary::host::HostEnvironment;
use crate::design::{Document, Inline, PromptUi, Ui, Warning};
use crate::diagnostics::ExitCode;
use crate::msg;
use crate::support::inventory;

use crate::commands::present;

use super::super::{Context, report};
use super::Args;

pub fn exec(
    args: &Args,
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    let (_config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    prompt.set_locale(locale);
    let prepared = match super::run::prepare(
        context.location,
        args.project.as_ref(),
        args.index,
        host,
        prompt,
        context.workspace_root,
        inventory::Poll::default(),
        ui,
    ) {
        Ok(prepared) => prepared,
        Err(error) => return report(ui, &error),
    };

    if let Some(index) = prepared.missing_worktree_index {
        ui.warning(&Warning::text(msg!(
            "warning-open-worktree-not-found",
            index = index
        )));
    }

    // promptで選べた値と接続先が食い違う場合は、接続先を見せる前にその差を述べる。
    if let Some(clamped) = prepared.clamped_worktree_index {
        ui.warning(&Warning::text(msg!(
            "warning-open-worktree-index-clamped",
            requested = clamped.requested,
            index = clamped.opened
        )));
    }

    // 接続先はterminalを引き渡す前に見せる。
    let connecting = Document::new()
        .summary(msg!(
            "open-connecting",
            project = prepared.project,
            sandbox = prepared.sandbox
        ))
        .lines(
            Some(msg!("open-worktrees-heading")),
            prepared
                .worktrees
                .iter()
                .map(Inline::path)
                .map(Into::into)
                .collect(),
        );
    ui.stderr(&present::disk_section(connecting, &prepared.disk));

    match super::run::connect(host, prepared, ui) {
        Ok(()) => ExitCode::Success,
        Err(error) => report(ui, &error),
    }
}

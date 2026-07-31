//! `open`の実行と出力。

use crate::command::RealHost;
use crate::design::{Document, Inline, Ui};
use crate::diagnostics::ExitCode;
use crate::msg;
use crate::project::ProjectId;
use crate::support::{inventory, sandbox};

use super::super::{Context, report};

pub fn exec(project: Option<&ProjectId>, context: &Context, ui: &mut Ui) -> ExitCode {
    let (_config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    let mut prompt = ui.prompt();
    let prepared = match super::run::prepare(
        context.location,
        project,
        &RealHost,
        &mut prompt,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        inventory::Poll::default(),
        ui,
    ) {
        Ok(prepared) => prepared,
        Err(error) => return report(ui, &error),
    };

    // 接続先はterminalを引き渡す前に見せる。
    ui.stderr(
        &Document::new()
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
            ),
    );

    match super::run::connect(&RealHost, &prepared) {
        Ok(()) => ExitCode::Success,
        Err(error) => report(ui, &error),
    }
}

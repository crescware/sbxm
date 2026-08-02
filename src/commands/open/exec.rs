//! `open`の実行と出力。

use crate::command::HostEnvironment;
use crate::design::{Document, Inline, PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::msg;
use crate::project::ProjectId;
use crate::support::{inventory, sandbox};

use super::super::{Context, report};

pub fn exec(
    project: Option<&ProjectId>,
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
        project,
        host,
        prompt,
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

    match super::run::connect(host, &prepared) {
        Ok(()) => ExitCode::Success,
        Err(error) => report(ui, &error),
    }
}

//! `open`の実行と出力。

use crate::boundary::host::HostEnvironment;
use crate::design::{Document, Inline, PromptUi, Ui, Warning};
use crate::diagnostics::ExitCode;
use crate::msg;

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
    let (config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    prompt.set_locale(locale);
    if let Err(error) = crate::support::sbx_cli::require_ready(context.location, host, ui) {
        return report(ui, &error);
    }
    if let Err(error) =
        crate::support::git_cli::require_supported_version(context.location, host, ui)
    {
        return report(ui, &error);
    }
    let prepared = match super::prepare_connection(args, context, &config, host, prompt, ui) {
        Ok(prepared) => prepared,
        Err(error) => return report(ui, &error),
    };

    // 初回構築を行った実行では、接続先を見せる前に何を作ったかを示す。stdoutはSSHへ
    // 引き渡すため、この報告もstderrへ出す。
    if let Some(output) = &prepared.provisioned {
        for warning in &output.warnings {
            ui.warning(warning);
        }
        ui.stderr(&present::provisioning_output(output, locale));
    }
    for warning in &prepared.warnings {
        ui.warning(warning);
    }

    if let Some(index) = prepared.missing_worktree_index {
        ui.warning(&Warning::text(msg!(
            "warning-open-worktree-not-found",
            index = index
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

    super::session::session(context, ui, host, prepared)
}

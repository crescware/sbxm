//! `destroy`の実行。
//!
//! 消す前に計画を見せ、確認を取ってから実行する。確認前の計画と実行後の結果は
//! 別のdocumentとして作り、同じ画面のなかで混ざらないようにする。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::support::{inventory, sandbox};
use crate::ui::Ui;

use super::super::{Context, report};
use super::run::TerminalConfirmPrompt;
use super::{Args, print};

pub fn exec(args: &Args, context: &Context, ui: &mut Ui) -> ExitCode {
    let (_config, locale) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    let mut prompt = ui.prompt();
    let prepared = match super::run::prepare(
        context.location,
        args.project.as_ref(),
        args.force,
        &RealHost,
        &mut prompt,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
    ) {
        Ok(prepared) => prepared,
        Err(error) => return report(ui, &error),
    };

    ui.stdout(&print::plan_document(&prepared.plan, locale));
    if prepared.plan.force {
        ui.warning(&print::force_notice());
    }

    let mut confirm = TerminalConfirmPrompt::new(ui.prompt());
    if let Err(error) =
        super::run::confirm(&prepared, context.interactivity.can_prompt(), &mut confirm)
    {
        return report(ui, &error);
    }
    ui.note_prompt_output();

    let outcome = match super::run::execute(&RealHost, &prepared, inventory::Poll::default(), ui) {
        Ok(outcome) => outcome,
        Err(error) => return report(ui, &error),
    };

    // project lockを手放してから、短時間だけregistry lockを取ってentryを外す。
    let canonical = prepared.canonical_id().clone();
    drop(prepared);
    if let Err(error) = super::run::unregister(context.location, &canonical) {
        return report(ui, &error);
    }

    for warning in &outcome.warnings {
        ui.warning(warning);
    }
    ui.stdout(&print::outcome_document(&outcome));
    ExitCode::Success
}

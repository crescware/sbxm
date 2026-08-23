//! `destroy`の実行。
//!
//! 消す前に計画を見せ、確認を取ってから実行する。確認前の計画と実行後の結果は
//! 別のdocumentとして作り、同じ画面のなかで混ざらないようにする。

use crate::boundary::host::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::support::inventory;

use super::{
    super::{Context, report},
    Args, print,
};

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
    let mut prepared = match super::run::prepare(
        context.location,
        args.project.as_ref(),
        args.force,
        host,
        prompt,
        context.workspace_root,
    ) {
        Ok(prepared) => prepared,
        Err(error) => return report(ui, &error),
    };

    ui.stdout(&print::plan_document(&prepared.plan, locale));
    if prepared.plan.force {
        ui.warning(&print::force_notice());
    }

    let confirmation = match super::run::confirm(&mut prepared, context.can_prompt, prompt) {
        Ok(confirmation) => confirmation,
        Err(error) => return report(ui, &error),
    };
    ui.note_prompt_output();

    let executed = match confirmation {
        Some(confirmation) => super::run::execute_confirmed(
            host,
            &prepared,
            confirmation,
            inventory::Poll::default(),
            ui,
        ),
        None => super::run::execute_bypassed(host, &prepared, inventory::Poll::default(), ui),
    };
    let mut outcome = match executed {
        Ok(outcome) => outcome,
        Err(error) => return report(ui, &error),
    };

    // project lockを手放してから、短時間だけregistry lockを取ってentryを外す。
    let unregistration = prepared.unregistration();
    drop(prepared);
    match super::run::unregister(context.location, &unregistration) {
        // 管理を解いた案件が登録し直されていれば、entryは残したまま報告する。
        Ok(kept) => outcome.warnings.extend(kept),
        Err(error) => return report(ui, &error),
    }

    for warning in &outcome.warnings {
        ui.warning(warning);
    }
    ui.stdout(&print::outcome_document(&outcome));
    ExitCode::Success
}

//! `destroy`の実行。
//!
//! 消す前に計画を見せ、確認を取ってから実行する。確認前の計画と実行後の結果は
//! 別のdocumentとして作り、同じ画面のなかで混ざらないようにする。originに無いcommitが
//! 削除を止めた場合は、hostのrepositoryへ保存してから続けるかを訊く。

use std::ops::ControlFlow;

use crate::boundary::host::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::msg;
use crate::support::{inventory, select};

use super::{
    super::{Context, fetch, report, saving},
    Args, Selection, print,
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
    if let Err(error) = crate::support::login::require_signed_in(host) {
        return report(ui, &error);
    }
    // hostへ保存してから準備をやり直す場合に、対象を選び直させない。
    let chosen = match select::chosen(
        context.location,
        args.project.as_ref(),
        &msg!("select-destroy-heading"),
        prompt,
    ) {
        Ok(chosen) => chosen,
        Err(error) => return report(ui, &error),
    };
    // hostにあるrepositoryの案件は、消す前に保存しておく。保護の検査を迂回する
    // `--force`でも、保存できるものは保存する。
    fetch::print::auto_saved(
        ui,
        &fetch::save_first(context.location, &chosen, host, context.workspace_root),
    );
    let prepared =
        saving::prepare_offering_save(&chosen, context, host, prompt, ui, |prompt, ui| {
            let selection = Selection {
                location: context.location,
                requested: Some(&chosen),
                prompt,
            };
            super::run::prepare(
                selection,
                args.force,
                host,
                context.workspace_root,
                inventory::Poll::default(),
                ui,
            )
        });
    let mut prepared = match prepared {
        ControlFlow::Continue(prepared) => prepared,
        ControlFlow::Break(code) => return code,
    };

    ui.stdout(&print::plan_document(&prepared.plan, locale));
    if prepared.plan.started {
        ui.warning(&print::started_notice());
    }
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

#[cfg(test)]
#[path = "exec_test.rs"]
mod exec_test;

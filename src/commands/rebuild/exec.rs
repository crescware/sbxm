//! `rebuild`の実行。
//!
//! 作り直す前に計画を見せ、確認を取ってから実行する。originに無いcommitが作り直しを
//! 止めた場合は、hostのrepositoryへ保存してから続けるかを訊く。

use std::ops::ControlFlow;

use crate::boundary::host::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::msg;
use crate::project::ProjectId;
use crate::support::{inventory, select};

use super::{
    super::{Context, report, saving},
    Target, print,
};

pub fn exec(
    project: Option<&ProjectId>,
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
    if let Err(error) = crate::support::login::require_signed_in(host) {
        return report(ui, &error);
    }
    // hostへ保存してから準備をやり直す場合に、対象を選び直させない。
    let chosen = match select::chosen(
        context.location,
        project,
        &msg!("select-rebuild-heading"),
        prompt,
    ) {
        Ok(chosen) => chosen,
        Err(error) => return report(ui, &error),
    };
    // hostにあるrepositoryの案件は、作り直す前に保存しておく。
    let saved = saving::save_first(context.location, &chosen, host, context.workspace_root, ui);
    saving::auto_saved(ui, &saved);
    let prepared =
        saving::prepare_offering_save(&chosen, context, host, prompt, ui, |prompt, ui| {
            let target = Target {
                location: context.location,
                requested: Some(&chosen),
                prompt,
            };
            super::run::prepare(
                target,
                host,
                context.workspace_root,
                inventory::Poll::default(),
                ui,
            )
        });
    let (prepared, snapshot) = match prepared {
        ControlFlow::Continue(pair) => pair,
        ControlFlow::Break(code) => return code,
    };

    ui.stdout(&print::plan_document(&prepared.plan));

    let confirmation =
        match super::run::confirm(snapshot, &prepared.plan.project, context.can_prompt, prompt) {
            Ok(confirmation) => confirmation,
            Err(error) => return report(ui, &error),
        };
    ui.note_prompt_output();

    match super::run::execute(
        host,
        prepared,
        confirmation,
        &config,
        context.workspace_root,
        inventory::Poll::default(),
        ui,
    ) {
        Ok(output) => print::report(ui, &output),
        Err(error) => report(ui, &error),
    }
}

#[cfg(test)]
#[path = "exec_test.rs"]
mod exec_test;

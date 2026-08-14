//! `rebuild`の実行。
//!
//! 作り直す前に計画を見せ、確認を取ってから実行する。

use crate::command::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::project::ProjectId;
use crate::support::inventory;

use super::{
    super::{Context, report},
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
    let target = Target {
        location: context.location,
        requested: project,
        prompt,
    };
    let (prepared, snapshot) = match super::run::prepare(
        target,
        host,
        context.workspace_root,
        inventory::Poll::default(),
        ui,
    ) {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };

    ui.stdout(&print::plan_document(&prepared.plan));

    let confirmation =
        match super::run::confirm(snapshot, context.interactivity.can_prompt(), prompt) {
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

//! `repair`の実行。

use crate::boundary::host::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::project::ProjectId;

use super::{
    super::{Context, report},
    print,
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

    let prepared = match super::run::prepare(
        context.location,
        &config,
        project,
        host,
        context.workspace_root,
        prompt,
    ) {
        Ok(prepared) => prepared,
        Err(error) => return report(ui, &error),
    };
    ui.stdout(&print::plan_document(&prepared.plan));

    match super::run::execute(host, prepared, &config, context.workspace_root, ui) {
        Ok(output) => {
            for warning in &output.warnings {
                ui.warning(warning);
            }
            ui.stdout(&print::document(&output));
            ExitCode::Success
        }
        Err(error) => report(ui, &error),
    }
}

//! `prepare`の実行。

use crate::command::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::project::ProjectId;
use crate::support::sandbox;

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
    match super::run::run(
        context.location,
        &config,
        project,
        host,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        prompt,
        ui,
    ) {
        Ok(output) => {
            for warning in &output.warnings {
                ui.warning(warning);
            }
            ui.stdout(&print::document(&output, locale));
            ExitCode::Success
        }
        Err(error) => report(ui, &error),
    }
}

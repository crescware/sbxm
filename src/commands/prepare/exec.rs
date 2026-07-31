//! `prepare`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::project::ProjectId;
use crate::support::sandbox;
use crate::ui::Ui;

use super::super::{Context, report};
use super::print;

pub fn exec(project: &ProjectId, context: &Context, ui: &mut Ui) -> ExitCode {
    let (config, locale) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    match super::run::run(
        context.location,
        &config,
        project,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
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

//! `add`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::ui::Ui;

use super::super::{Context, report};
use super::run::AddRequest;
use super::{Args, print};

pub fn exec(args: &Args, context: &Context, ui: &mut Ui) -> ExitCode {
    let (config, locale) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    let request = AddRequest {
        project: args.project.clone(),
        worktrees: args.worktrees,
        detach: args.detach.clone(),
    };
    match super::run::run(&config, &request, &RealHost, ui) {
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

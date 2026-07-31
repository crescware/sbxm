//! `apply`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::support::sandbox;
use crate::ui::Ui;

use super::super::{Context, report};
use super::run::Scope;
use super::{Args, print};

pub fn exec(args: &Args, context: &Context, ui: &mut Ui) -> ExitCode {
    let (config, locale) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    let scope = Scope {
        files: args.files,
        worktrees: args.worktrees,
    };
    match super::run::run(
        context.location,
        &config,
        &args.project,
        scope,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        ui,
    ) {
        Ok(output) => {
            ui.stdout(&print::document(&output, locale));
            ExitCode::Success
        }
        Err(error) => report(ui, &error),
    }
}

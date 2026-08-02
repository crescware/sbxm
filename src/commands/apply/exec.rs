//! `apply`の実行。

use crate::command::HostEnvironment;
use crate::design::Ui;
use crate::diagnostics::ExitCode;
use crate::support::sandbox;

use super::{
    super::{Context, report},
    Args, Scope, print,
};

pub fn exec(args: &Args, context: &Context, ui: &mut Ui, host: &dyn HostEnvironment) -> ExitCode {
    let (config, locale) = match context.settings() {
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
        host,
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

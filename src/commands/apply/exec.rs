//! `apply`の実行。

use crate::command::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::support::sandbox;

use super::{
    super::{Context, report},
    Args, Scope, Target, print,
};

pub fn exec(
    args: &Args,
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
    let scope = Scope {
        files: args.files,
        worktrees: args.worktrees,
    };
    let target = Target {
        location: context.location,
        requested: args.project.as_ref(),
        prompt,
    };
    match super::run::run(
        target,
        &config,
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

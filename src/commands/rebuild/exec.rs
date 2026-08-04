//! `rebuild`の実行。

use crate::command::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::project::ProjectId;
use crate::support::{inventory, sandbox};

use super::{
    super::{Context, report},
    print,
    run::Target,
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
    match super::run::run(
        target,
        &config,
        host,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        inventory::Poll::default(),
        ui,
    ) {
        Ok(output) => print::report(ui, &output),
        Err(error) => report(ui, &error),
    }
}

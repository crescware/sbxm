//! `stop`の実行。

use crate::command::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::project::ProjectId;
use crate::support::{inventory, sandbox};

use super::{
    super::{Context, report},
    print,
};

pub fn exec(
    projects: &[ProjectId],
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    let (_config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    prompt.set_locale(locale);
    match super::run::run(
        context.location,
        projects,
        host,
        prompt,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        inventory::Poll::default(),
    ) {
        Ok(stopped) => print::report(ui, &stopped),
        Err(error) => report(ui, &error),
    }
}

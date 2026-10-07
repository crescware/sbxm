//! `stop`の実行。

use crate::boundary::host::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::project::ProjectId;
use crate::support::inventory;

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
    if let Err(error) = crate::support::login::require_signed_in(host, ui) {
        return report(ui, &error);
    }
    match super::run::run(
        context.location,
        projects,
        host,
        prompt,
        context.workspace_root,
        inventory::Poll::standard(context.clock),
        ui,
    ) {
        Ok(stopped) => print::report(ui, &stopped),
        Err(error) => report(ui, &error),
    }
}

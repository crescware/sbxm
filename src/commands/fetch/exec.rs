//! `fetch`の実行。

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
    let (_config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    prompt.set_locale(locale);
    if let Err(error) = crate::support::login::require_signed_in(host) {
        return report(ui, &error);
    }
    match super::run(
        context.location,
        project,
        prompt,
        host,
        context.workspace_root,
    ) {
        Ok(output) => {
            ui.stdout(&print::document(&output, locale));
            ExitCode::Success
        }
        Err(error) => report(ui, &error),
    }
}

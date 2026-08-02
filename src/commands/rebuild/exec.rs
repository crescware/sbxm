//! `rebuild`の実行。

use crate::command::RealHost;
use crate::design::Ui;
use crate::diagnostics::ExitCode;
use crate::project::ProjectId;
use crate::support::{inventory, sandbox};

use super::{
    super::{Context, report},
    print,
};

pub fn exec(project: &ProjectId, context: &Context, ui: &mut Ui) -> ExitCode {
    let (config, locale) = match context.settings() {
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
        inventory::Poll::default(),
        ui,
    ) {
        Ok(output) => print::report(ui, &output),
        Err(error) => report(ui, &error),
    }
}

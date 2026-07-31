//! `stop`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::project::ProjectId;
use crate::support::{inventory, sandbox};
use crate::ui::Ui;

use super::super::{Context, report};
use super::print;

pub fn exec(projects: &[ProjectId], context: &Context, ui: &mut Ui) -> ExitCode {
    let (_config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    let mut prompt = ui.prompt();
    match super::run::run(
        context.location,
        projects,
        &RealHost,
        &mut prompt,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        inventory::Poll::default(),
    ) {
        Ok(stopped) => print::report(ui, &stopped),
        Err(error) => report(ui, &error),
    }
}

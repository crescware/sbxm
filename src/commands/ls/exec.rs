//! `ls`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::support::sandbox;
use crate::ui::Ui;

use super::super::{Context, report};
use super::print;

pub fn exec(context: &Context, ui: &mut Ui) -> ExitCode {
    let (config, locale) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    match super::run::run(
        &config,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
    ) {
        Ok(listing) => {
            ui.stdout(&print::document(&listing, locale));
            ExitCode::Success
        }
        Err(error) => report(ui, &error),
    }
}

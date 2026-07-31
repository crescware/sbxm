//! `ls`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::support::sandbox;
use crate::ui::Ui;

use super::super::{Context, report};
use super::print;

pub fn exec(context: &Context, ui: &mut Ui) -> ExitCode {
    let (_config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    match super::run::run(
        context.location,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
    ) {
        Ok(listing) => {
            ui.stdout(&print::document(&listing, locale));
            // 復旧に必要なentryをすべて見せたうえで、1件でも整っていなければ失敗とする。
            if listing.settled {
                ExitCode::Success
            } else {
                ExitCode::Failure
            }
        }
        Err(error) => report(ui, &error),
    }
}

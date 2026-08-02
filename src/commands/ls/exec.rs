//! `ls`の実行。

use crate::command::HostEnvironment;
use crate::design::Ui;
use crate::diagnostics::ExitCode;
use crate::support::sandbox;

use super::{
    super::{Context, report},
    print,
};

pub fn exec(context: &Context, ui: &mut Ui, host: &dyn HostEnvironment) -> ExitCode {
    let (_config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    match super::run::run(
        context.location,
        host,
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

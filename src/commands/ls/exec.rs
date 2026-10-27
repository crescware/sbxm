//! `ls`の実行。

use crate::boundary::host::HostEnvironment;
use crate::design::Ui;
use crate::diagnostics::ExitCode;

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
    // loginを確かめた一覧から組み立てる。lockもpromptも挟まないため、読み直さない。
    let entries = match crate::support::sbx_cli::require_ready(host, ui) {
        Ok(entries) => entries,
        Err(error) => return report(ui, &error),
    };
    match super::run::run(context.location, &entries, context.workspace_root, ui) {
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

#[cfg(test)]
#[path = "exec_test.rs"]
mod exec_test;

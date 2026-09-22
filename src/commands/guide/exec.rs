use crate::boundary::host::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;

use super::{Args, print};
use crate::commands::{Context, report};

pub fn exec(
    args: &Args,
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

    // secret一覧を読むcommandであるため、topicや案件を選ばせる前に認証を確認する。
    if let Err(error) = crate::support::login::require_signed_in(host) {
        return report(ui, &error);
    }

    match super::run::run(args, context.location, locale, host, prompt) {
        Ok(output) => {
            ui.stdout(&print::document(&output));
            ExitCode::Success
        }
        Err(error) => report(ui, &error),
    }
}

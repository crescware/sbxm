use crate::cli::{self, Interactivity, Outcome, PeekedLang};
use crate::command::RealHost;
use crate::commands::Context;
use crate::config::ConfigLocation;
use crate::design::{Document, Environment, OutputPolicy, PromptUi, Terminals, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;

use super::resolve_display_locale;

pub(crate) fn run(argv: &[String]) -> ExitCode {
    let peeked = cli::peek_lang(argv);
    let policy = OutputPolicy::resolve(
        cli::peek_color(argv),
        &Environment::detect(),
        &Terminals::detect(),
    );

    let location = match ConfigLocation::discover() {
        Ok(location) => location,
        Err(error) => {
            // localeが決まる前の失敗も、同じ描画条件で報告する。
            let mut ui = Ui::terminal(Locale::SOURCE, policy);
            ui.error(&error);
            return error.exit_code();
        }
    };

    let display_locale = resolve_display_locale::resolve_display_locale(&peeked, &location);
    let mut ui = Ui::terminal(display_locale, policy);

    // 表示localeはconfigからbest-effortで解決済みである。`--lang`の不正はconfigの
    // validation errorより先に報告するため、壊れたconfigがparse errorを覆い隠さない。
    if let PeekedLang::Invalid(value) = &peeked {
        let error = cli::invalid_lang_error(value);
        ui.error(&error);
        return error.exit_code();
    }

    let interactivity = Interactivity::detect();
    let catalog = crate::i18n::Catalog::new(display_locale);
    match cli::parse(argv, &catalog, interactivity) {
        Ok(Outcome::Help(text)) => {
            ui.help(&text);
            ExitCode::Success
        }
        Ok(Outcome::Version(text)) => {
            ui.stdout(&Document::new().verbatim(text));
            ExitCode::Success
        }
        Ok(Outcome::Run(command)) => {
            let context = Context {
                location: &location,
                workspace_root: std::path::Path::new(crate::support::sandbox::WORKSPACE_ROOT),
                lang: match peeked {
                    PeekedLang::Valid(locale) => Some(locale),
                    _ => None,
                },
                interactivity,
            };
            // 実hostと実端末と実workspace rootを選ぶのはここだけとする。commandは受け取った
            // ものだけを使う。
            let mut prompt = PromptUi::terminal(display_locale, policy.stderr);
            crate::commands::dispatch(&command, &context, &mut ui, &RealHost, &mut prompt)
        }
        Err(error) => {
            ui.error(&error);
            error.exit_code()
        }
    }
}

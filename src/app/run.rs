use crate::cli::{self, Interactivity, Outcome};
use crate::command::RealHost;
use crate::commands::Context;
use crate::config::{self, ConfigLocation, ConfigState};
use crate::design::{Document, PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;

pub(crate) fn run(argv: Vec<String>) -> ExitCode {
    let invocation = cli::Invocation::new(argv);
    let command_line_locale = invocation.command_line_locale();
    let policy = RenderingPolicy::detect(invocation.color());

    let location = match ConfigLocation::discover() {
        Ok(location) => location,
        Err(error) => {
            // localeが決まる前の失敗も、同じ描画条件で報告する。
            let mut ui = Ui::terminal(Locale::SOURCE, policy);
            ui.error(&error);
            return error.exit_code();
        }
    };

    let configured_locale = match config::load(&location) {
        Ok(ConfigState::Valid { config, .. }) => config.language,
        _ => None,
    };
    let display_locale = crate::i18n::resolve_display_locale(
        command_line_locale,
        configured_locale,
        crate::i18n::shell_locale(),
    );
    let mut ui = Ui::terminal(display_locale, policy);

    // 表示localeはconfigからbest-effortで解決済みである。`--lang`の不正はconfigの
    // validation errorより先に報告するため、壊れたconfigがparse errorを覆い隠さない。
    if let Some(value) = invocation.invalid_language() {
        let error = cli::invalid_lang_error(value);
        ui.error(&error);
        return error.exit_code();
    }

    let interactivity = Interactivity::detect();
    let catalog = crate::i18n::Catalog::new(display_locale);
    match invocation.parse(&catalog, interactivity) {
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
                lang: command_line_locale,
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

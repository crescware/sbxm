//! sbxm。案件ごとのDocker Sandboxを構築、接続、診断、破棄するCLI。
//!
//! 実行順は、引数validation、config load、project解決、外部command、mutationとする。
//! commandの実装は`commands`が1 command 1 directoryで持ち、本fileはlocale決定、描画条件の
//! 決定、引き渡しだけを行う。
//!
//! 利用者向けの描画はすべて`design`が行う。本fileはstreamへ直接書かない。

mod archive;
mod cli;
mod command;
mod commands;
mod compatibility;
mod config;
mod design;
mod diagnostics;
mod git;
mod hash;
mod i18n;
mod image_labels;
mod metadata;
mod paths;
mod project;
mod registry;
mod repository;
mod support;
#[cfg(test)]
mod testing;

use std::process::ExitCode as ProcessExitCode;

use cli::{Interactivity, Outcome, PeekedLang};
use command::RealHost;
use commands::Context;
use config::{ConfigLocation, ConfigState};
use design::{Document, Environment, OutputPolicy, PromptUi, Terminals, Ui};
use diagnostics::ExitCode;
use i18n::{Locale, shell_locale};

fn main() -> ProcessExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let code = run(&argv);
    ProcessExitCode::from(code.as_u8())
}

fn run(argv: &[String]) -> ExitCode {
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

    let display_locale = resolve_display_locale(&peeked, &location);
    let mut ui = Ui::terminal(display_locale, policy);

    // 表示localeはconfigからbest-effortで解決済みである。`--lang`の不正はconfigの
    // validation errorより先に報告するため、壊れたconfigがparse errorを覆い隠さない。
    if let PeekedLang::Invalid(value) = &peeked {
        let error = cli::invalid_lang_error(value);
        ui.error(&error);
        return error.exit_code();
    }

    let interactivity = Interactivity::detect();
    let catalog = i18n::Catalog::new(display_locale);
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
                workspace_root: std::path::Path::new(support::sandbox::WORKSPACE_ROOT),
                lang: match peeked {
                    PeekedLang::Valid(locale) => Some(locale),
                    _ => None,
                },
                interactivity,
            };
            // 実hostと実端末と実workspace rootを選ぶのはここだけとする。commandは受け取った
            // ものだけを使う。
            let mut prompt = PromptUi::terminal(display_locale, policy.stderr);
            commands::dispatch(&command, &context, &mut ui, &RealHost, &mut prompt)
        }
        Err(error) => {
            ui.error(&error);
            error.exit_code()
        }
    }
}

/// helpとusageのlocaleを決める。
///
/// 1. argvから先読みした有効な`--lang`
/// 2. read-onlyかつbest-effortで読み込めた有効なglobal configの`language`
/// 3. shell locale
/// 4. `en`
fn resolve_display_locale(peeked: &PeekedLang, location: &ConfigLocation) -> Locale {
    if let PeekedLang::Valid(locale) = peeked {
        return *locale;
    }
    // configが不在、構文不正、未知version、permission不正、symlink、read失敗のいずれでも
    // help表示自体は妨げず、shell localeへfallbackする。
    if let Ok(ConfigState::Valid { config, .. }) = config::load(location)
        && let Some(locale) = config.language
    {
        return locale;
    }
    shell_locale().unwrap_or(Locale::SOURCE)
}

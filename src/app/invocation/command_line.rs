//! 生argvと、完全parseより前に必要な先読み値。

use crate::design::{ColorMode, ColorSetting};
use crate::i18n::Locale;

mod color;
mod lang;
mod peek;

use lang::PeekedLang;
use peek::peek;

/// 1回のCLI呼び出しに渡されたcommand line。
pub(crate) struct CommandLine {
    argv: Vec<String>,
    locale_override: PeekedLang,
    color_setting: ColorSetting,
}

impl CommandLine {
    pub(crate) fn new(argv: Vec<String>) -> Self {
        let locale_override = peek(&argv, lang::OPTION_NAME).map_or(PeekedLang::Absent, |value| {
            match Locale::parse_exact(value) {
                Some(locale) => PeekedLang::Valid(locale),
                None => PeekedLang::Invalid(value.to_owned()),
            }
        });
        let color_setting = peek(&argv, color::OPTION_NAME)
            .and_then(ColorMode::parse_exact)
            .map(ColorSetting::Explicit)
            .unwrap_or_default();

        Self {
            argv,
            locale_override,
            color_setting,
        }
    }

    pub(crate) fn argv(&self) -> &[String] {
        &self.argv
    }

    pub(crate) fn locale_override(&self) -> Option<Locale> {
        match self.locale_override {
            PeekedLang::Valid(locale) => Some(locale),
            PeekedLang::Absent | PeekedLang::Invalid(_) => None,
        }
    }

    pub(crate) fn invalid_locale_override(&self) -> Option<&str> {
        match &self.locale_override {
            PeekedLang::Invalid(value) => Some(value),
            PeekedLang::Absent | PeekedLang::Valid(_) => None,
        }
    }

    pub(crate) fn color_setting(&self) -> ColorSetting {
        self.color_setting
    }

    pub(super) fn invalid_locale_error(value: &str) -> crate::diagnostics::Error {
        lang::invalid_lang_error(value)
    }

    pub(super) fn color_arg(
        builder: &super::parse::help::Builder,
    ) -> crate::diagnostics::Result<clap::Arg> {
        color::arg(builder)
    }

    pub(super) fn lang_arg(
        builder: &super::parse::help::Builder,
    ) -> crate::diagnostics::Result<clap::Arg> {
        lang::arg(builder)
    }
}

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

#[cfg(test)]
#[path = "command_line/peek_test.rs"]
mod peek_test;

//! 生argvと、完全parseより前に必要な先読み値。

use crate::design::{ColorMode, ColorSetting};
use crate::i18n::Locale;

use super::invalid_lang_error;
use super::locale_override::LocaleOverride;
use super::peek::peek;
use super::preparse_option::PreparseOption;

/// 1回のCLI呼び出しに渡されたcommand line。
pub(crate) struct CommandLine {
    argv: Vec<String>,
    locale_override: LocaleOverride,
    color_setting: ColorSetting,
}

impl CommandLine {
    pub(crate) fn new(argv: Vec<String>) -> Self {
        let locale_override =
            peek(&argv, PreparseOption::Lang).map_or(LocaleOverride::Absent, |value| {
                match Locale::parse_exact(value) {
                    Some(locale) => LocaleOverride::Valid(locale),
                    None => LocaleOverride::Invalid(value.to_owned()),
                }
            });
        let color_setting = peek(&argv, PreparseOption::Color)
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
            LocaleOverride::Valid(locale) => Some(locale),
            LocaleOverride::Absent | LocaleOverride::Invalid(_) => None,
        }
    }

    pub(crate) fn invalid_locale_override(&self) -> Option<&str> {
        match &self.locale_override {
            LocaleOverride::Invalid(value) => Some(value),
            LocaleOverride::Absent | LocaleOverride::Valid(_) => None,
        }
    }

    pub(crate) fn color_setting(&self) -> ColorSetting {
        self.color_setting
    }

    pub(crate) fn invalid_locale_error(value: &str) -> crate::diagnostics::Error {
        invalid_lang_error::invalid_lang_error(value)
    }
}

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

#[cfg(test)]
#[path = "peek_test.rs"]
mod peek_test;

#[cfg(test)]
#[path = "invalid_lang_error_test.rs"]
mod invalid_lang_error_test;

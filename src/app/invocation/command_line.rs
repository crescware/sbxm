//! 生argvと、完全parseより前に必要な先読み値。

use crate::design::ColorMode;
use crate::i18n::Locale;

mod color;
mod lang;

use color::peek_color;
use lang::{PeekedLang, peek_lang};

/// 1回のCLI呼び出しに渡されたcommand line。
pub(crate) struct CommandLine {
    argv: Vec<String>,
    locale_override: PeekedLang,
    color_mode: ColorMode,
}

impl CommandLine {
    pub(crate) fn new(argv: Vec<String>) -> Self {
        Self {
            locale_override: peek_lang(&argv),
            color_mode: peek_color(&argv),
            argv,
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

    pub(crate) fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    pub(super) fn invalid_locale_error(&self, value: &str) -> crate::diagnostics::Error {
        lang::invalid_lang_error(value)
    }

    pub(super) fn color_arg(
        builder: &crate::app::invocation::help::Builder,
    ) -> crate::diagnostics::Result<clap::Arg> {
        color::arg(builder)
    }

    pub(super) fn lang_arg(
        builder: &crate::app::invocation::help::Builder,
    ) -> crate::diagnostics::Result<clap::Arg> {
        lang::arg(builder)
    }
}

#[cfg(test)]
#[path = "command_line_test.rs"]
mod command_line_test;

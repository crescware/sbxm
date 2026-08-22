use crate::design::ColorMode;
use crate::diagnostics::Result;
use crate::i18n::{Catalog, Locale};

use super::color::peek_color;
use super::lang::{PeekedLang, peek_lang};
use super::parse::parse as parse_argv;
use super::{Interactivity, Outcome};

/// 1回のCLI呼び出しと、helpを組み立てる前に必要な表示用option。
pub struct Invocation {
    argv: Vec<String>,
    language: PeekedLang,
    color: ColorMode,
}

impl Invocation {
    pub fn new(argv: Vec<String>) -> Invocation {
        let language = peek_lang(&argv);
        let color = peek_color(&argv);
        Invocation {
            argv,
            language,
            color,
        }
    }

    pub fn command_line_locale(&self) -> Option<Locale> {
        match self.language {
            PeekedLang::Valid(locale) => Some(locale),
            PeekedLang::Absent | PeekedLang::Invalid(_) => None,
        }
    }

    pub fn color(&self) -> ColorMode {
        self.color
    }

    pub fn invalid_language(&self) -> Option<&str> {
        match &self.language {
            PeekedLang::Invalid(value) => Some(value),
            PeekedLang::Absent | PeekedLang::Valid(_) => None,
        }
    }

    pub fn parse(self, catalog: &Catalog, interactivity: Interactivity) -> Result<Outcome> {
        parse_argv(&self.argv, catalog, interactivity)
    }
}

#[cfg(test)]
#[path = "invocation_test.rs"]
mod invocation_test;

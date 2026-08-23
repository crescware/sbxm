//! `--lang`。
//!
//! 受け付ける値と表記は[`crate::i18n::Locale`]が提供し、本moduleはCLI adapterとdiagnosticだけを持つ。
//! 言語を増やしてもこのmoduleを触らない。

mod arg;
mod invalid_lang_error;
mod option_name;

pub use arg::arg;
pub use invalid_lang_error::invalid_lang_error;
pub(super) use option_name::OPTION_NAME;

#[cfg(test)]
#[path = "lang_test.rs"]
mod lang_test;

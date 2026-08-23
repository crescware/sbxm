use crate::diagnostics::{Error, ErrorId};
use crate::i18n::Locale;
use crate::msg;

/// `--lang`の不正値に対するerror。configのvalidationより先に報告する。
pub fn invalid_lang_error(value: &str) -> Error {
    Error::new(
        ErrorId::InvalidLang,
        msg!(
            "error-invalid-lang",
            value = value,
            supported = Locale::value_list()
        ),
    )
}

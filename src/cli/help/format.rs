use crate::diagnostics::{Error, ErrorId, Msg, Result};
use crate::i18n::Catalog;
use crate::msg;

pub(super) fn format(catalog: &Catalog, message: &Msg) -> Result<String> {
    catalog.format(message).map_err(|failure| {
        Error::new(
            ErrorId::MessageFormatFailed,
            msg!("error-invalid-arguments").with("detail", failure),
        )
    })
}

#[cfg(test)]
#[path = "format_test.rs"]
mod format_test;

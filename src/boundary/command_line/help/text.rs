use crate::diagnostics::Result;
use crate::i18n::Catalog;
use crate::msg;

use super::format;

pub(super) fn text(catalog: &Catalog, id: &'static str) -> Result<String> {
    format(catalog, &msg!(id))
}

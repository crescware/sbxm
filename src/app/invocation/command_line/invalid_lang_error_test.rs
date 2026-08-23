use crate::diagnostics::ErrorId;

use super::invalid_lang_error::invalid_lang_error;

#[test]
fn an_unsupported_language_is_rejected_without_reading_anything_else() {
    let error = invalid_lang_error("zz");
    assert_eq!(error.first_id(), Some(ErrorId::InvalidLang));
}

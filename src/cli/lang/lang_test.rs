use crate::diagnostics::ErrorId;
use crate::i18n::Locale;

use super::*;
use crate::testing::cli::argv;

#[test]
fn lang_is_read_before_the_parser_runs_and_from_either_side() {
    assert_eq!(
        peek_lang(&argv(&["--lang", "ja", "init"])),
        PeekedLang::Valid(Locale::Ja)
    );
    assert_eq!(
        peek_lang(&argv(&["init", "--lang", "ja"])),
        PeekedLang::Valid(Locale::Ja)
    );
    assert_eq!(
        peek_lang(&argv(&["--lang=en", "ls"])),
        PeekedLang::Valid(Locale::En)
    );
    assert_eq!(peek_lang(&argv(&["ls"])), PeekedLang::Absent);
    assert_eq!(
        peek_lang(&argv(&["--lang", "zz", "ls"])),
        PeekedLang::Invalid("zz".to_string())
    );
    // `--`以降は先読みしない。
    assert_eq!(
        peek_lang(&argv(&["ls", "--", "--lang", "ja"])),
        PeekedLang::Absent
    );
}

#[test]
fn an_unsupported_language_is_rejected_without_reading_anything_else() {
    let error = invalid_lang_error("zz");
    assert_eq!(error.first_id(), Some(ErrorId::InvalidLang));
}

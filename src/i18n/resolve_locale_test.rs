use super::super::{Locale, resolve_locale};

#[test]
fn command_line_language_takes_precedence() {
    assert_eq!(
        resolve_locale(Some(Locale::Ja), Some(Locale::En), Some(Locale::En)),
        Locale::Ja
    );
}

#[test]
fn configured_language_is_used_without_a_command_line_language() {
    assert_eq!(
        resolve_locale(None, Some(Locale::Ja), Some(Locale::En)),
        Locale::Ja
    );
}

#[test]
fn shell_language_is_the_fallback() {
    assert_eq!(resolve_locale(None, None, Some(Locale::Ja)), Locale::Ja);
}

#[test]
fn the_source_locale_is_the_last_resort() {
    assert_eq!(resolve_locale(None, None, None), Locale::SOURCE);
}

use super::super::{Locale, resolve_display_locale};

#[test]
fn command_line_locale_takes_precedence() {
    assert_eq!(
        resolve_display_locale(Some(Locale::Ja), Some(Locale::En), Some(Locale::En)),
        Locale::Ja
    );
}

#[test]
fn configured_locale_takes_precedence_over_shell_locale() {
    assert_eq!(
        resolve_display_locale(None, Some(Locale::Ja), Some(Locale::En)),
        Locale::Ja
    );
}

#[test]
fn shell_locale_is_used_when_no_higher_priority_locale_exists() {
    assert_eq!(
        resolve_display_locale(None, None, Some(Locale::Ja)),
        Locale::Ja
    );
}

#[test]
fn source_locale_is_used_when_no_locale_exists() {
    assert_eq!(resolve_display_locale(None, None, None), Locale::SOURCE);
}

use crate::design::ColorMode;
use crate::i18n::Locale;
use crate::testing::cli::argv;

use super::Invocation;

#[test]
fn an_invocation_captures_the_options_needed_before_full_parsing() {
    let invocation = Invocation::new(argv(&["--lang", "ja", "--color=never", "ls"]));

    assert_eq!(invocation.command_line_locale(), Some(Locale::Ja));
    assert_eq!(invocation.color(), ColorMode::Never);
}

#[test]
fn an_invalid_language_remains_available_for_the_parser_diagnostic() {
    let invocation = Invocation::new(argv(&["--lang=zz", "ls"]));

    assert_eq!(invocation.command_line_locale(), None);
    assert_eq!(invocation.invalid_language(), Some("zz"));
}

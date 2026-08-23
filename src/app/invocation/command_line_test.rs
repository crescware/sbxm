use crate::design::{ColorMode, ColorSetting};
use crate::i18n::Locale;
use crate::testing::cli::argv;

use super::CommandLine;

#[test]
fn a_command_line_captures_options_needed_before_full_parsing() {
    let command_line = CommandLine::new(argv(&["--lang", "ja", "--color=never", "ls"]));

    assert_eq!(command_line.locale_override(), Some(Locale::Ja));
    assert_eq!(
        command_line.color_setting(),
        ColorSetting::Explicit(ColorMode::Never)
    );
}

#[test]
fn an_invalid_language_remains_available_for_the_application_diagnostic() {
    let command_line = CommandLine::new(argv(&["--lang=zz", "ls"]));

    assert_eq!(command_line.locale_override(), None);
    assert_eq!(command_line.invalid_locale_override(), Some("zz"));
}

#[test]
fn an_invalid_color_is_left_for_the_parser_to_reject() {
    let command_line = CommandLine::new(argv(&["--color=maybe", "ls"]));

    assert_eq!(command_line.color_setting(), ColorSetting::Default);
}

#[test]
fn options_are_peeked_before_or_after_the_subcommand_in_both_forms() {
    for arguments in [
        vec!["--lang=ja", "--color=never", "ls"],
        vec!["ls", "--lang", "ja", "--color", "never"],
    ] {
        let command_line = CommandLine::new(argv(&arguments));
        assert_eq!(command_line.locale_override(), Some(Locale::Ja));
        assert_eq!(
            command_line.color_setting(),
            ColorSetting::Explicit(ColorMode::Never)
        );
    }
}

#[test]
fn options_after_double_dash_are_not_peeked() {
    let command_line = CommandLine::new(argv(&["ls", "--", "--lang=ja", "--color=never"]));

    assert_eq!(command_line.locale_override(), None);
    assert_eq!(command_line.color_setting(), ColorSetting::Default);
}

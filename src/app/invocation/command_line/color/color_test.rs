use crate::design::{ColorMode, ColorSetting};

use super::*;

fn argv(arguments: &[&str]) -> Vec<String> {
    std::iter::once("sbxm")
        .chain(arguments.iter().copied())
        .map(str::to_string)
        .collect()
}

#[test]
fn no_option_leaves_the_decision_to_the_environment() {
    assert_eq!(peek_color(&argv(&["ls"])), ColorSetting::Default);
}

#[test]
fn both_spellings_of_the_option_are_read() {
    assert_eq!(
        peek_color(&argv(&["--color=never", "ls"])),
        ColorSetting::Explicit(ColorMode::Never)
    );
    assert_eq!(
        peek_color(&argv(&["--color", "always", "ls"])),
        ColorSetting::Explicit(ColorMode::Always)
    );
}

#[test]
fn the_option_is_read_after_the_subcommand_as_well() {
    // globalなoptionであり、subcommandの後ろでも同じ意味になる。
    assert_eq!(
        peek_color(&argv(&["status", "--global", "--color=always"])),
        ColorSetting::Explicit(ColorMode::Always)
    );
}

#[test]
fn an_unsupported_value_is_left_for_the_parser_to_reject() {
    // 先読みは描画条件を決めるためだけに行い、validationの順序を変えない。
    assert_eq!(
        peek_color(&argv(&["--color=maybe", "ls"])),
        ColorSetting::Default
    );
}

#[test]
fn nothing_after_the_separator_is_read_as_an_option() {
    assert_eq!(
        peek_color(&argv(&["ls", "--", "--color=always"])),
        ColorSetting::Default
    );
}

#[test]
fn an_explicit_auto_is_distinct_from_no_option() {
    assert_eq!(
        peek_color(&argv(&["--color=auto", "ls"])),
        ColorSetting::Explicit(ColorMode::Auto)
    );
}

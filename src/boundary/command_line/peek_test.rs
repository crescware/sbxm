use crate::testing::cli::argv;

use super::{PreparseOption, peek};

#[test]
fn no_option_returns_no_value() {
    assert_eq!(peek(&argv(&["ls"]), PreparseOption::Color), None);
}

#[test]
fn both_long_option_spellings_are_read() {
    assert_eq!(
        peek(&argv(&["--color=never", "ls"]), PreparseOption::Color),
        Some("never")
    );
    assert_eq!(
        peek(&argv(&["--lang", "ja", "ls"]), PreparseOption::Lang),
        Some("ja")
    );
}

#[test]
fn an_option_is_read_after_the_subcommand() {
    assert_eq!(
        peek(
            &argv(&["status", "--global", "--color=always"]),
            PreparseOption::Color
        ),
        Some("always")
    );
}

#[test]
fn the_first_matching_option_wins() {
    assert_eq!(
        peek(
            &argv(&["--color=never", "--color", "always", "ls"]),
            PreparseOption::Color
        ),
        Some("never")
    );
}

#[test]
fn an_unsupported_value_is_still_returned_for_the_caller() {
    assert_eq!(
        peek(&argv(&["--color=maybe", "ls"]), PreparseOption::Color),
        Some("maybe")
    );
}

#[test]
fn an_option_name_is_matched_exactly() {
    assert_eq!(
        peek(&argv(&["--colorful=always", "ls"]), PreparseOption::Color),
        None
    );
}

#[test]
fn nothing_after_the_separator_is_read_as_an_option() {
    assert_eq!(
        peek(
            &argv(&["ls", "--", "--color=always"]),
            PreparseOption::Color
        ),
        None
    );
}

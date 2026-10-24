use crate::testing::cli::argv;

use super::{PreparseOption, peek_flag};

#[test]
fn an_absent_flag_is_not_seen() {
    assert!(!peek_flag(&argv(&["ls"]), PreparseOption::Verbose));
}

#[test]
fn the_flag_is_seen_before_or_after_the_subcommand() {
    assert!(peek_flag(
        &argv(&["--verbose", "ls"]),
        PreparseOption::Verbose
    ));
    assert!(peek_flag(
        &argv(&["status", "--global", "--verbose"]),
        PreparseOption::Verbose
    ));
}

#[test]
fn a_flag_name_is_matched_exactly() {
    assert!(!peek_flag(
        &argv(&["--verbosely", "ls"]),
        PreparseOption::Verbose
    ));
    assert!(!peek_flag(
        &argv(&["--verbose=1", "ls"]),
        PreparseOption::Verbose
    ));
    assert!(!peek_flag(
        &argv(&["-verbose", "ls"]),
        PreparseOption::Verbose
    ));
}

#[test]
fn nothing_after_the_separator_is_read_as_a_flag() {
    assert!(!peek_flag(
        &argv(&["ls", "--", "--verbose"]),
        PreparseOption::Verbose
    ));
}

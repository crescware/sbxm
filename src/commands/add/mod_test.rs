use super::*;
use crate::commands::Command;
use crate::testing::cli::{command, parse_argv, tty};

#[test]
fn worktree_counts_outside_the_allowed_range_are_refused() {
    for value in ["0", "33", "999", "abc", ""] {
        let error = parse_argv(&["add", "owner/repo", "--worktrees", value], tty())
            .expect_err("{value} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::WorktreesOutOfRange),
            "value {value} produced the wrong error"
        );
    }
    // 負値はoptionとして解釈されるため、値の範囲ではなくsyntaxの段階で止まる。
    let error = parse_argv(&["add", "owner/repo", "--worktrees", "-1"], tty())
        .expect_err("a negative count never reaches the range check");
    assert_eq!(error.exit_code(), crate::error::ExitCode::Failure);
    assert!(matches!(
        command(&["add", "owner/repo", "--worktrees", "1"], tty()),
        Command::Add(Args {
            worktrees: Some(1),
            ..
        })
    ));
    assert!(matches!(
        command(
            &[
                "add",
                "owner/repo",
                "--worktrees",
                "32",
                "--detach",
                "develop"
            ],
            tty()
        ),
        Command::Add(Args {
            worktrees: Some(32),
            ..
        })
    ));
}

#[test]
fn more_than_one_worktree_requires_an_explicit_start_branch() {
    let error = parse_argv(&["add", "owner/repo", "--worktrees", "2"], tty())
        .expect_err("two worktrees without a branch are refused");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesRequireDetach));

    assert!(matches!(
        command(
            &[
                "add",
                "owner/repo",
                "--worktrees",
                "2",
                "--detach",
                "develop"
            ],
            tty()
        ),
        Command::Add(_)
    ));
}

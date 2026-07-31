use super::*;
use crate::commands::Command;
use crate::testing::cli::{command, parse_argv, tty};

#[test]
fn worktree_counts_outside_the_allowed_range_are_refused() {
    for value in ["0", "33", "999", "abc", ""] {
        let error = parse_argv(
            &["add", "git@github.com:owner/repo.git", "--worktrees", value],
            tty(),
        )
        .expect_err("{value} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::WorktreesOutOfRange),
            "value {value} produced the wrong error"
        );
    }
    // 負値はoptionとして解釈されるため、値の範囲ではなくsyntaxの段階で止まる。
    let error = parse_argv(
        &["add", "git@github.com:owner/repo.git", "--worktrees", "-1"],
        tty(),
    )
    .expect_err("a negative count never reaches the range check");
    assert_eq!(error.exit_code(), crate::error::ExitCode::Failure);
    assert!(matches!(
        command(
            &["add", "git@github.com:owner/repo.git", "--worktrees", "1"],
            tty()
        ),
        Command::Add(Args {
            worktrees: Some(1),
            ..
        })
    ));
    assert!(matches!(
        command(
            &[
                "add",
                "git@github.com:owner/repo.git",
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

/// `-t`は`--worktrees`の別名であり、同じ本数として解釈される。
#[test]
fn the_short_form_requests_the_same_worktree_count() {
    assert!(matches!(
        command(
            &[
                "add",
                "git@github.com:owner/repo.git",
                "-t",
                "3",
                "--detach",
                "develop"
            ],
            tty()
        ),
        Command::Add(Args {
            worktrees: Some(3),
            ..
        })
    ));
}

#[test]
fn more_than_one_worktree_requires_an_explicit_start_branch() {
    let error = parse_argv(
        &["add", "git@github.com:owner/repo.git", "--worktrees", "2"],
        tty(),
    )
    .expect_err("two worktrees without a branch are refused");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesRequireDetach));

    assert!(matches!(
        command(
            &[
                "add",
                "git@github.com:owner/repo.git",
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

#[test]
fn a_declared_identity_is_carried_only_when_both_halves_are_given() {
    assert!(matches!(
        command(
            &[
                "add",
                "git@github.com:owner/repo.git",
                "--git-user-name",
                "Example User",
                "--git-user-email",
                "user@example.com",
            ],
            tty()
        ),
        Command::Add(Args {
            git_identity: Some(_),
            ..
        })
    ));

    // 宣言が無いことは、既定かpromptで決めるという意味であり、errorではない。
    assert!(matches!(
        command(&["add", "git@github.com:owner/repo.git"], tty()),
        Command::Add(Args {
            git_identity: None,
            ..
        })
    ));

    // 片方だけは不完全な意図である。足りないoption名を示して止まる。
    for (option, value) in [
        ("--git-user-name", "Example User"),
        ("--git-user-email", "user@example.com"),
    ] {
        let error = parse_argv(
            &["add", "git@github.com:owner/repo.git", option, value],
            tty(),
        )
        .expect_err("{option} alone must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::GitIdentityIncomplete),
            "{option} produced the wrong error"
        );
    }
}

#[test]
fn a_declared_value_that_git_cannot_use_is_refused() {
    for (name, email) in [("", "user@example.com"), ("Example User", "  ")] {
        let error = parse_argv(
            &[
                "add",
                "git@github.com:owner/repo.git",
                "--git-user-name",
                name,
                "--git-user-email",
                email,
            ],
            tty(),
        )
        .expect_err("{name:?} {email:?} must be refused");
        assert_eq!(error.first_id(), Some(ErrorId::InvalidValue));
    }
}

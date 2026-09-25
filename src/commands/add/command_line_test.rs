use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused};

use crate::commands::{Command, add::Args};
use crate::metadata::MAX_WORKTREES;
use crate::testing::cli::{command, parse_argv, tty};

#[test]
fn worktree_counts_outside_the_allowed_range_are_refused() -> Checked {
    for value in ["0", "33", "999", "abc", ""] {
        let error = parse_argv(
            &["add", "git@github.com:owner/repo.git", "--worktrees", value],
            tty(),
        )
        .refused_because("{value} must be refused")?;
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
    .refused_because("a negative count never reaches the range check")?;
    assert_eq!(error.exit_code(), crate::diagnostics::ExitCode::Failure);
    assert!(matches!(
        command(
            &["add", "git@github.com:owner/repo.git", "--worktrees", "1"],
            tty()
        )?,
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
                &MAX_WORKTREES.to_string(),
                "--detach",
                "develop"
            ],
            tty()
        )?,
        Command::Add(Args {
            worktrees: Some(MAX_WORKTREES),
            ..
        })
    ));
    Ok(())
}

/// `-t`は`--worktrees`の別名であり、同じ本数として解釈される。
#[test]
fn the_short_form_requests_the_same_worktree_count() -> Checked {
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
        )?,
        Command::Add(Args {
            worktrees: Some(3),
            ..
        })
    ));
    Ok(())
}

#[test]
fn more_than_one_worktree_requires_an_explicit_start_branch() -> Checked {
    let error = parse_argv(
        &["add", "git@github.com:owner/repo.git", "--worktrees", "2"],
        tty(),
    )
    .refused_because("two worktrees without a branch are refused")?;
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
        )?,
        Command::Add(_)
    ));
    Ok(())
}

#[test]
fn a_declared_identity_is_carried_only_when_both_halves_are_given() -> Checked {
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
        )?,
        Command::Add(Args {
            git_identity: Some(_),
            ..
        })
    ));

    // 宣言が無いことは、既定かpromptで決めるという意味であり、errorではない。
    assert!(matches!(
        command(&["add", "git@github.com:owner/repo.git"], tty())?,
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
        .refused_because("{option} alone must be refused")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::GitIdentityIncomplete),
            "{option} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_declared_value_that_git_cannot_use_is_refused() -> Checked {
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
        .refused_because("{name:?} {email:?} must be refused")?;
        assert_eq!(error.first_id(), Some(ErrorId::InvalidValue));
    }
    Ok(())
}

#[test]
fn a_repository_on_the_host_is_added_by_its_path_and_an_optional_name() -> Checked {
    use crate::commands::add::AddTarget;
    use std::path::PathBuf;

    assert!(matches!(
        command(&["add", "--local", "../app/.git"], tty())?,
        Command::Add(Args {
            target: AddTarget::Local { path, name: None },
            ..
        }) if path == PathBuf::from("../app/.git")
    ));
    assert!(matches!(
        command(&["add", "--local", "/code/app/.git", "--name", "tool"], tty())?,
        Command::Add(Args {
            target: AddTarget::Local { name: Some(name), .. },
            ..
        }) if name == "tool"
    ));
    Ok(())
}

#[test]
fn a_clone_url_and_a_host_path_cannot_be_added_together() -> Checked {
    let error = parse_argv(
        &[
            "add",
            "git@github.com:owner/repo.git",
            "--local",
            "/code/app/.git",
        ],
        tty(),
    )
    .refused_because("two repositories")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConflictingArguments));

    let error = parse_argv(
        &["add", "git@github.com:owner/repo.git", "--name", "tool"],
        tty(),
    )
    .refused_because("a name only belongs to a host repository")?;
    assert_eq!(error.first_id(), Some(ErrorId::NameWithoutLocal));

    // clone URLだけを求めると、hostにあるrepositoryも登録できることが伝わらない。
    let error = parse_argv(&["add"], tty()).refused_because("nothing to add")?;
    assert_eq!(error.first_id(), Some(ErrorId::MissingRequiredArgument));
    assert_eq!(
        error.diagnostics()[0].description,
        crate::msg!(
            "error-missing-required-argument",
            argument = "<github-clone-url> | --local <GIT_DIR>"
        )
    );
    Ok(())
}

#[test]
fn an_empty_host_path_is_refused_rather_than_read_as_the_current_directory() -> Checked {
    // 空のpathはcwdと同じ場所へ解決される。書き忘れた値を、cwdの登録として受け取らない。
    let error = parse_argv(&["add", "--local", ""], tty()).refused_because("no path")?;
    assert_eq!(error.first_id(), Some(ErrorId::MissingRequiredArgument));
    Ok(())
}

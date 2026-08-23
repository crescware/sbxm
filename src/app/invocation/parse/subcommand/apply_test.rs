use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused};

use crate::commands::{Command, apply::Args};
use crate::testing::cli::{command, parse_argv, tty};

#[test]
fn apply_requires_an_explicit_scope() -> Checked {
    let error = parse_argv(&["apply", "owner/repo"], tty())
        .refused_because("apply without a scope is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ApplyScopeRequired));

    assert!(matches!(
        command(&["apply", "owner/repo", "--files"], tty())?,
        Command::Apply(Args {
            files: true,
            worktrees: None,
            ..
        })
    ));
    assert!(matches!(
        command(&["apply", "owner/repo", "--worktrees", "3"], tty())?,
        Command::Apply(Args {
            files: false,
            worktrees: Some(3),
            ..
        })
    ));
    assert!(matches!(
        command(
            &["apply", "owner/repo", "--files", "--worktrees", "3"],
            tty()
        )?,
        Command::Apply(Args {
            files: true,
            worktrees: Some(3),
            ..
        })
    ));
    Ok(())
}

/// `-t`は`--worktrees`の別名であり、同じ本数として解釈される。
#[test]
fn the_short_form_requests_the_same_worktree_count() -> Checked {
    assert!(matches!(
        command(&["apply", "owner/repo", "-t", "3"], tty())?,
        Command::Apply(Args {
            files: false,
            worktrees: Some(3),
            ..
        })
    ));
    Ok(())
}

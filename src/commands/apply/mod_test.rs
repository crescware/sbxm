use super::*;
use crate::commands::Command;
use crate::testing::cli::{command, parse_argv, tty};

#[test]
fn apply_requires_an_explicit_scope() {
    let error =
        parse_argv(&["apply", "owner/repo"], tty()).expect_err("apply without a scope is refused");
    assert_eq!(error.first_id(), Some(ErrorId::ApplyScopeRequired));

    assert!(matches!(
        command(&["apply", "owner/repo", "--files"], tty()),
        Command::Apply(Args {
            files: true,
            worktrees: None,
            ..
        })
    ));
    assert!(matches!(
        command(&["apply", "owner/repo", "--worktrees", "3"], tty()),
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
        ),
        Command::Apply(Args {
            files: true,
            worktrees: Some(3),
            ..
        })
    ));
}

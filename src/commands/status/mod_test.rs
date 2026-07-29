use super::*;
use crate::commands::Command;
use crate::testing::cli::{command, parse_argv, tty};

#[test]
fn status_requires_exactly_one_scope() {
    assert_eq!(
        command(&["status", "--global"], tty()),
        Command::Status(Scope::Global)
    );
    assert_eq!(
        command(&["status", "-g"], tty()),
        Command::Status(Scope::Global)
    );
    assert!(matches!(
        command(&["status", "owner/repo"], tty()),
        Command::Status(Scope::Project(_))
    ));

    for arguments in [vec!["status"], vec!["status", "--global", "owner/repo"]] {
        let error = parse_argv(&arguments, tty()).expect_err("exactly one scope is required");
        assert_eq!(error.first_id(), Some(ErrorId::StatusScopeRequired));
    }
}

#[test]
fn status_never_prompts_even_on_a_terminal() {
    let error = parse_argv(&["status"], tty()).expect_err("status does not offer a project prompt");
    assert_eq!(error.first_id(), Some(ErrorId::StatusScopeRequired));
}

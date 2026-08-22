use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};

use crate::commands::{Command, status::Scope};
use crate::testing::cli::{command, non_tty, parse_argv, tty};

#[test]
fn status_requires_a_scope_outside_a_terminal() -> Checked {
    assert_eq!(
        command(&["status", "--global"], tty())?,
        Command::Status(Scope::Global)
    );
    assert_eq!(
        command(&["status", "-g"], tty())?,
        Command::Status(Scope::Global)
    );
    assert!(matches!(
        command(&["status", "owner/repo"], tty())?,
        Command::Status(Scope::Project(_))
    ));

    let error = parse_argv(&["status"], non_tty())
        .refused_because("status without a scope needs a terminal for the selection prompt")?;
    assert_eq!(error.first_id(), Some(ErrorId::StatusScopeRequired));

    let error = parse_argv(&["status", "--global", "owner/repo"], tty())
        .refused_because("exactly one scope is required")?;
    assert_eq!(error.first_id(), Some(ErrorId::StatusScopeRequired));
    Ok(())
}

#[test]
fn status_without_a_scope_prompts_on_a_terminal() -> Checked {
    assert_eq!(command(&["status"], tty())?, Command::Status(Scope::Prompt));
    Ok(())
}

#[test]
fn status_with_a_project_or_global_scope_does_not_prompt() -> Checked {
    for arguments in [vec!["status", "owner/repo"], vec!["status", "--global"]] {
        let command = parse_argv(&arguments, tty()).required_because("an explicit scope parses")?;
        assert!(matches!(command, Command::Status(_)));
    }
    Ok(())
}

use std::path::PathBuf;

use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused};

use crate::commands::{Command, files::Args};
use crate::testing::cli::{command, non_tty, parse_argv, tty};

#[test]
fn each_action_is_read_with_the_values_it_takes() -> Checked {
    assert_eq!(
        command(&["files", "add", "/Users/you/.claude/CLAUDE.md"], non_tty())?,
        Command::Files(Args::Add {
            source: PathBuf::from("/Users/you/.claude/CLAUDE.md"),
            destination: None,
        })
    );
    assert_eq!(
        command(
            &["files", "add", "notes.md", "--dest", ".config/notes.md"],
            tty()
        )?,
        Command::Files(Args::Add {
            source: PathBuf::from("notes.md"),
            destination: Some(".config/notes.md".to_string()),
        })
    );
    assert_eq!(
        command(&["files", "ls"], non_tty())?,
        Command::Files(Args::Ls)
    );
    assert_eq!(
        command(&["files", "rm", ".claude/CLAUDE.md"], non_tty())?,
        Command::Files(Args::Rm {
            destination: ".claude/CLAUDE.md".to_string()
        })
    );
    Ok(())
}

#[test]
fn an_action_without_what_it_needs_or_with_what_it_ignores_is_refused() -> Checked {
    for (arguments, expected) in [
        (vec!["files"], ErrorId::MissingRequiredArgument),
        (vec!["files", "add"], ErrorId::MissingRequiredArgument),
        (vec!["files", "rm"], ErrorId::MissingRequiredArgument),
        (vec!["files", "sync"], ErrorId::InvalidValue),
        (vec!["files", "ls", "extra"], ErrorId::UnknownArgument),
        // 配置先を選べるのは足すときだけである。
        (
            vec!["files", "ls", "--dest", "x"],
            ErrorId::ConflictingArguments,
        ),
        (
            vec!["files", "rm", ".gitconfig", "--dest", "x"],
            ErrorId::ConflictingArguments,
        ),
    ] {
        let error =
            parse_argv(&arguments, tty()).refused_because(&format!("{arguments:?} is refused"))?;
        assert_eq!(error.first_id(), Some(expected), "{arguments:?}");
    }
    Ok(())
}

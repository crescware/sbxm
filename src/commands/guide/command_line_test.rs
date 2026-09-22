use crate::commands::{Command, guide};
use crate::diagnostics::ErrorId;
use crate::testing::cli::{command, non_tty, parse_argv, tty};
use crate::testing::outcome::{Checked, Refused};

#[test]
fn the_topic_and_project_can_both_be_explicit() -> Checked {
    assert!(matches!(
        command(&["guide", "credential-rotation", "owner/repo"], non_tty())?,
        Command::Guide(guide::Args {
            topic: Some(guide::Topic::CredentialRotation),
            project: Some(_),
        })
    ));
    Ok(())
}

#[test]
fn an_explicit_topic_can_leave_the_project_to_the_prompt() -> Checked {
    assert_eq!(
        command(&["guide", "credential-rotation"], tty())?,
        Command::Guide(guide::Args {
            topic: Some(guide::Topic::CredentialRotation),
            project: None,
        })
    );

    let error = parse_argv(&["guide", "credential-rotation"], non_tty())
        .refused_because("a project cannot be chosen without a terminal")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectArgumentRequired));
    Ok(())
}

#[test]
fn an_interactive_run_can_choose_both_topic_and_project() -> Checked {
    assert_eq!(
        command(&["guide"], tty())?,
        Command::Guide(guide::Args {
            topic: None,
            project: None,
        })
    );

    let error = parse_argv(&["guide"], non_tty())
        .refused_because("a topic cannot be chosen without a terminal")?;
    assert_eq!(error.first_id(), Some(ErrorId::MissingRequiredArgument));
    Ok(())
}

#[test]
fn an_unknown_topic_is_named_as_an_invalid_value() -> Checked {
    let error = parse_argv(&["guide", "unknown", "owner/repo"], non_tty())
        .refused_because("only declared guide topics are accepted")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidValue));
    assert_eq!(
        error.diagnostics()[0].description.args,
        [
            ("argument", "<topic>".to_string()),
            ("value", "unknown".to_string())
        ]
    );
    Ok(())
}

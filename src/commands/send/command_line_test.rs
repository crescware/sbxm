use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused};

use crate::commands::Command;
use crate::testing::cli::{command, non_tty, parse_argv, tty};

#[test]
fn send_takes_the_project_or_asks_for_it_on_a_terminal() -> Checked {
    assert_eq!(
        command(&["send", "local/app"], non_tty())?,
        Command::Send(Some(crate::project::ProjectId::parse("local/app")?))
    );
    assert_eq!(command(&["send"], tty())?, Command::Send(None));
    let error = parse_argv(&["send"], non_tty())
        .refused_because("there is nobody to choose the project")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectArgumentRequired));
    Ok(())
}

use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused};

use crate::commands::Command;
use crate::testing::cli::{command, non_tty, parse_argv, tty};

#[test]
fn fetch_takes_the_project_or_asks_for_it_on_a_terminal() -> Checked {
    assert_eq!(
        command(&["fetch", "owner/repo"], non_tty())?,
        Command::Fetch(Some(crate::project::ProjectId::parse("owner/repo")?))
    );
    assert_eq!(command(&["fetch"], tty())?, Command::Fetch(None));
    let error = parse_argv(&["fetch"], non_tty())
        .refused_because("there is nobody to choose the project")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectArgumentRequired));
    Ok(())
}

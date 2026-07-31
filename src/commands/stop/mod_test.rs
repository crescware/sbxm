use crate::testing::outcome::{Checked, Required};

use super::*;
use crate::commands::Command;
use crate::testing::cli::{command, non_tty};

#[test]
fn stop_accepts_several_projects() -> Checked {
    assert_eq!(
        command(&["stop", "owner/one", "owner/two"], non_tty())?,
        Command::Stop(vec![
            ProjectId::parse("owner/one").required()?,
            ProjectId::parse("owner/two").required()?,
        ])
    );
    Ok(())
}

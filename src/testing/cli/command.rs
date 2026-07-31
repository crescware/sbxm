use crate::testing::outcome::{Checked, Required, Unmet};

use crate::cli::{Interactivity, Outcome};
use crate::commands::Command;

use super::parse_argv;

/// parseが成功し、commandを返すことを前提に取り出す。
pub fn command(arguments: &[&str], interactivity: Interactivity) -> Checked<Command> {
    Ok(
        match parse_argv(arguments, interactivity).required_because("the arguments parse")? {
            Outcome::Run(command) => command,
            other => return Err(Unmet::new(format!("expected a command, got {other:?}"))),
        },
    )
}

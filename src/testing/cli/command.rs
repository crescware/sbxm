use crate::testing::outcome::{Checked, Required};

use crate::app::Interactivity;
use crate::commands::Command;

use super::parse_argv;

/// parseが成功し、commandを返すことを前提に取り出す。
pub fn command(arguments: &[&str], interactivity: Interactivity) -> Checked<Command> {
    parse_argv(arguments, interactivity).required_because("the arguments parse")
}

use clap::error::ErrorKind;

use crate::commands::Command;
use crate::diagnostics::Result;

use super::map;

pub(crate) fn interpret(error: &clap::Error) -> Result<Command> {
    match error.kind() {
        // helpとversionはexit code `0`。libraryの既定exit codeは透過しない。
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            Ok(Command::Help(error.render().to_string()))
        }
        ErrorKind::DisplayVersion => {
            Ok(Command::Version(super::super::version_line::version_line()))
        }
        _ => Err(map(error)),
    }
}

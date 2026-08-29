use clap::error::ErrorKind;

use crate::boundary::command_line::ParsedCommandLine;
use crate::diagnostics::Result;

use super::map::map;

pub(crate) fn interpret(error: &clap::Error) -> Result<ParsedCommandLine> {
    match error.kind() {
        // helpとversionはexit code `0`。libraryの既定exit codeは透過しない。
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            Ok(ParsedCommandLine::Help(error.render().to_string()))
        }
        ErrorKind::DisplayVersion => Ok(ParsedCommandLine::Version(
            super::super::version_line::version_line(),
        )),
        _ => Err(map(error)),
    }
}

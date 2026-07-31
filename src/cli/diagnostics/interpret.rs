use clap::error::ErrorKind;

use crate::diagnostics::Result;

use crate::cli::Outcome;

use super::map;

pub fn interpret(error: &clap::Error) -> Result<Outcome> {
    match error.kind() {
        // helpとversionはexit code `0`。libraryの既定exit codeは透過しない。
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            Ok(Outcome::Help(error.render().to_string()))
        }
        ErrorKind::DisplayVersion => Ok(Outcome::Version(crate::cli::version_line())),
        _ => Err(map(error)),
    }
}

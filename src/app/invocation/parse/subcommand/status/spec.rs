use clap::{Arg, ArgAction, Command as ClapCommand};

use super::super::super::super::help::Builder;
use super::super::super::project_arg::PROJECT_VALUE_NAME;
use crate::diagnostics::Result;

pub(crate) fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder
        .positional("status", "cli-status-about")?
        .arg(
            Arg::new("project")
                .value_name(PROJECT_VALUE_NAME)
                .help(builder.text("cli-status-project-help")?),
        )
        .arg(
            Arg::new("global")
                .long("global")
                .short('g')
                .action(ArgAction::SetTrue)
                .help(builder.text("cli-status-global-help")?),
        ))
}

use clap::{Arg, ArgAction, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::project_arg::PROJECT_VALUE_NAME;
use crate::diagnostics::Result;

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
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

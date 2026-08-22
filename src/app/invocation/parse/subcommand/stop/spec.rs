use clap::{Arg, ArgAction, Command as ClapCommand};

use super::super::super::super::help::Builder;
use super::super::super::project_arg::PROJECT_VALUE_NAME;
use crate::diagnostics::Result;

pub(crate) fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder.positional("stop", "cli-stop-about")?.arg(
        Arg::new("project")
            .value_name(PROJECT_VALUE_NAME)
            .num_args(0..)
            .action(ArgAction::Append)
            .help(builder.text("cli-stop-project-help")?),
    ))
}

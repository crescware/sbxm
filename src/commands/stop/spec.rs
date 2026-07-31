use clap::{Arg, ArgAction, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::project_arg::PROJECT_VALUE_NAME;
use crate::diagnostics::Result;

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder.positional("stop", "cli-stop-about")?.arg(
        Arg::new("project")
            .value_name(PROJECT_VALUE_NAME)
            .num_args(0..)
            .action(ArgAction::Append)
            .help(builder.text("cli-stop-project-help")?),
    ))
}

use clap::{Arg, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::project_arg::PROJECT_VALUE_NAME;
use crate::diagnostics::Result;

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder.positional("prepare", "cli-prepare-about")?.arg(
        Arg::new("project")
            .value_name(PROJECT_VALUE_NAME)
            .help(builder.text("cli-prepare-project-help")?),
    ))
}

use clap::{Arg, Command as ClapCommand};

use super::super::super::super::help::Builder;
use super::super::super::project_arg::PROJECT_VALUE_NAME;
use crate::diagnostics::Result;

pub(crate) fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder.positional("prepare", "cli-prepare-about")?.arg(
        Arg::new("project")
            .value_name(PROJECT_VALUE_NAME)
            .help(builder.text("cli-prepare-project-help")?),
    ))
}

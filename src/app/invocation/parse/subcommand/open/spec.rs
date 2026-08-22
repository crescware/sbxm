use clap::{Arg, Command as ClapCommand};

use super::super::super::super::help::Builder;
use super::super::super::project_arg::PROJECT_VALUE_NAME;
use crate::diagnostics::Result;

pub(crate) fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder
        .positional("open", "cli-open-about")?
        .arg(
            Arg::new("project")
                .value_name(PROJECT_VALUE_NAME)
                .help(builder.text("cli-open-project-help")?),
        )
        .arg(
            Arg::new("index")
                .long("index")
                .short('i')
                .value_name("N")
                .value_parser(clap::value_parser!(u32))
                .help(builder.text("cli-open-index-help")?),
        ))
}

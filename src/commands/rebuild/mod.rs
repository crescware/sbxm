//! `sbxm rebuild`。

mod exec;
pub mod run;

pub use exec::exec;

use clap::{Arg, ArgMatches, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::project_arg::{PROJECT_VALUE_NAME, required_project};
use crate::error::Result;
use crate::project::ProjectId;

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder.positional("rebuild", "cli-rebuild-about")?.arg(
        Arg::new("project")
            .required(true)
            .value_name(PROJECT_VALUE_NAME)
            .help(builder.text("cli-rebuild-project-help")?),
    ))
}

pub fn args(matches: &ArgMatches) -> Result<ProjectId> {
    required_project(matches)
}

//! `sbxm open`。

mod exec;
pub mod run;

pub use exec::exec;

use clap::{Arg, ArgMatches, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::Interactivity;
use crate::cli::project_arg::{PROJECT_VALUE_NAME, optional_project};
use crate::error::Result;
use crate::project::ProjectId;

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder.positional("open", "cli-open-about")?.arg(
        Arg::new("project")
            .value_name(PROJECT_VALUE_NAME)
            .help(builder.text("cli-open-project-help")?),
    ))
}

pub fn args(matches: &ArgMatches, interactivity: Interactivity) -> Result<Option<ProjectId>> {
    optional_project(matches, interactivity, "sbxm open")
}

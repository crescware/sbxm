//! `sbxm stop`。

mod exec;
pub mod print;
pub mod run;

pub use exec::exec;

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::Interactivity;
use crate::cli::project_arg::{PROJECT_VALUE_NAME, require_prompt_capability};
use crate::error::Result;
use crate::project::ProjectId;

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder.positional("stop", "cli-stop-about")?.arg(
        Arg::new("project")
            .value_name(PROJECT_VALUE_NAME)
            .num_args(0..)
            .action(ArgAction::Append)
            .help(builder.text("cli-stop-project-help")?),
    ))
}

pub fn args(matches: &ArgMatches, interactivity: Interactivity) -> Result<Vec<ProjectId>> {
    let values: Vec<String> = matches
        .get_many::<String>("project")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    if values.is_empty() {
        require_prompt_capability(interactivity, "sbxm stop")?;
        return Ok(Vec::new());
    }
    let mut projects = Vec::with_capacity(values.len());
    for value in values {
        projects.push(ProjectId::parse(&value)?);
    }
    Ok(projects)
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;

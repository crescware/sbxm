use clap::ArgMatches;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::project::ProjectId;

pub fn required_project(matches: &ArgMatches) -> Result<ProjectId> {
    let value = matches.get_one::<String>("project").ok_or_else(|| {
        Error::new(
            ErrorId::MissingRequiredArgument,
            msg!("error-missing-required-argument", argument = "<project-id>"),
        )
    })?;
    ProjectId::parse(value)
}

#[cfg(test)]
#[path = "required_project_test.rs"]
mod required_project_test;

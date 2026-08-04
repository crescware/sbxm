use clap::ArgMatches;

use crate::cli::Interactivity;
use crate::cli::project_arg::optional_project;
use crate::diagnostics::Result;
use crate::project::ProjectId;

pub fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Option<ProjectId>> {
    optional_project(matches, interactivity, "sbxm rebuild")
}

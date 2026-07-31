use clap::ArgMatches;

use crate::cli::project_arg::required_project;
use crate::diagnostics::Result;
use crate::project::ProjectId;

pub fn parse(matches: &ArgMatches) -> Result<ProjectId> {
    required_project(matches)
}

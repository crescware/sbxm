use clap::ArgMatches;

use crate::app::invocation::Interactivity;
use crate::diagnostics::Result;
use crate::project::ProjectId;

pub(crate) fn parse(
    matches: &ArgMatches,
    interactivity: Interactivity,
) -> Result<Option<ProjectId>> {
    super::super::super::project_arg::optional_project(matches, interactivity, "sbxm rebuild")
}

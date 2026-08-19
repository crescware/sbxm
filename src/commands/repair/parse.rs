use clap::ArgMatches;

use crate::cli::Interactivity;
use crate::cli::project_arg::optional_project;
use crate::diagnostics::Result;
use crate::project::ProjectId;

/// repair対象を引数または対話promptから読む。
pub fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Option<ProjectId>> {
    optional_project(matches, interactivity, "sbxm repair")
}

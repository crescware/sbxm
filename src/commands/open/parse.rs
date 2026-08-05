use clap::ArgMatches;

use crate::cli::Interactivity;
use crate::cli::project_arg::optional_project;
use crate::diagnostics::Result;

use super::Args;

pub fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Args> {
    Ok(Args {
        project: optional_project(matches, interactivity, "sbxm open")?,
        index: matches.get_one::<u32>("index").copied(),
    })
}

use clap::ArgMatches;

use crate::app::invocation::Interactivity;
use crate::commands::open::Args;
use crate::diagnostics::Result;

pub(crate) fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Args> {
    Ok(Args {
        project: super::super::super::project_arg::optional_project(
            matches,
            interactivity,
            "sbxm open",
        )?,
        index: matches.get_one::<u32>("index").copied(),
    })
}

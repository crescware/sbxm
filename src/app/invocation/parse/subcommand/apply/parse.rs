use clap::ArgMatches;

use crate::app::invocation::Interactivity;
use crate::commands::apply::Args;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

pub(crate) fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Args> {
    let files = matches.get_flag("files");
    let worktrees = matches.get_one::<u32>("worktrees").copied();
    if !files && worktrees.is_none() {
        return fail(
            ErrorId::ApplyScopeRequired,
            msg!("error-apply-scope-required"),
        );
    }
    Ok(Args {
        project: super::super::super::project_arg::optional_project(
            matches,
            interactivity,
            "sbxm apply",
        )?,
        files,
        worktrees,
    })
}

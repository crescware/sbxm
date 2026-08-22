use clap::ArgMatches;

use crate::app::invocation::Interactivity;
use crate::commands::status::Scope;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

pub(crate) fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Scope> {
    let global = matches.get_flag("global");
    let project = matches.get_one::<String>("project");
    match (global, project) {
        (true, None) => Ok(Scope::Global),
        (false, Some(value)) => Ok(Scope::Project(ProjectId::parse(value)?)),
        (false, None) if interactivity.can_prompt() => Ok(Scope::Prompt),
        _ => fail(
            ErrorId::StatusScopeRequired,
            msg!("error-status-scope-required"),
        ),
    }
}

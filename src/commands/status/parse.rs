use clap::ArgMatches;

use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

use super::Scope;

pub fn parse(matches: &ArgMatches) -> Result<Scope> {
    let global = matches.get_flag("global");
    let project = matches.get_one::<String>("project");
    match (global, project) {
        (true, None) => Ok(Scope::Global),
        (false, Some(value)) => Ok(Scope::Project(ProjectId::parse(value)?)),
        _ => fail(
            ErrorId::StatusScopeRequired,
            msg!("error-status-scope-required"),
        ),
    }
}

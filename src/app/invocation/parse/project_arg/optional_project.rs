use clap::ArgMatches;

use crate::diagnostics::Result;
use crate::project::ProjectId;

use crate::app::invocation::Interactivity;

use super::require_prompt_capability;

pub fn optional_project(
    matches: &ArgMatches,
    interactivity: Interactivity,
    command: &str,
) -> Result<Option<ProjectId>> {
    if let Some(value) = matches.get_one::<String>("project") {
        Ok(Some(ProjectId::parse(value)?))
    } else {
        require_prompt_capability(interactivity, command)?;
        Ok(None)
    }
}

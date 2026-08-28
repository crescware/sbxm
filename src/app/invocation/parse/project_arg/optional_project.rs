use clap::ArgMatches;

use crate::boundary::terminal::PromptCapability;
use crate::diagnostics::Result;
use crate::project::ProjectId;

use super::require_prompt_capability;

pub fn optional_project(
    matches: &ArgMatches,
    prompt: PromptCapability,
    command: &str,
) -> Result<Option<ProjectId>> {
    if let Some(value) = matches.get_one::<String>("project") {
        Ok(Some(ProjectId::parse(value)?))
    } else {
        require_prompt_capability(prompt, command)?;
        Ok(None)
    }
}

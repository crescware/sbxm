use clap::ArgMatches;

use crate::cli::Interactivity;
use crate::cli::project_arg::require_prompt_capability;
use crate::diagnostics::Result;
use crate::project::ProjectId;

pub fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Vec<ProjectId>> {
    let values: Vec<String> = matches
        .get_many::<String>("project")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    if values.is_empty() {
        require_prompt_capability(interactivity, "sbxm stop")?;
        return Ok(Vec::new());
    }
    let mut projects = Vec::with_capacity(values.len());
    for value in values {
        projects.push(ProjectId::parse(&value)?);
    }
    Ok(projects)
}

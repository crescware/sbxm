use clap::ArgMatches;

use crate::app::invocation::Interactivity;
use crate::diagnostics::Result;
use crate::project::ProjectId;

pub(crate) fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Vec<ProjectId>> {
    let values: Vec<String> = matches
        .get_many::<String>("project")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    if values.is_empty() {
        super::super::super::project_arg::require_prompt_capability(interactivity, "sbxm stop")?;
        return Ok(Vec::new());
    }
    values
        .into_iter()
        .map(|value| ProjectId::parse(&value))
        .collect()
}

//! `stop`のcommand-line adapter。

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::boundary::terminal::PromptCapability;
use crate::diagnostics::Result;
use crate::project::ProjectId;

use super::super::help::Builder;
use super::super::project_arg;

pub(super) struct Stop;

impl Stop {
    pub(super) fn spec(builder: &Builder) -> Result<ClapCommand> {
        Ok(builder.positional("stop", "cli-stop-about")?.arg(
            Arg::new("project")
                .value_name(project_arg::PROJECT_VALUE_NAME)
                .num_args(0..)
                .action(ArgAction::Append)
                .help(builder.text("cli-stop-project-help")?),
        ))
    }

    pub(super) fn parse(matches: &ArgMatches, prompt: PromptCapability) -> Result<Vec<ProjectId>> {
        let values: Vec<String> = matches
            .get_many::<String>("project")
            .map(|values| values.cloned().collect())
            .unwrap_or_default();
        if values.is_empty() {
            project_arg::require_prompt_capability(prompt, "sbxm stop")?;
            return Ok(Vec::new());
        }
        values
            .into_iter()
            .map(|value| ProjectId::parse(&value))
            .collect()
    }
}

#[cfg(test)]
#[path = "stop_test.rs"]
mod stop_test;

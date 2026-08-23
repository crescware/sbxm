//! `status`のcommand-line adapter。

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::app::invocation::Interactivity;
use crate::commands::status::Scope;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

use super::super::help::Builder;
use super::super::project_arg;

pub(super) struct Status;

impl Status {
    pub(super) fn spec(builder: &Builder) -> Result<ClapCommand> {
        Ok(builder
            .positional("status", "cli-status-about")?
            .arg(
                Arg::new("project")
                    .value_name(project_arg::PROJECT_VALUE_NAME)
                    .help(builder.text("cli-status-project-help")?),
            )
            .arg(
                Arg::new("global")
                    .long("global")
                    .short('g')
                    .action(ArgAction::SetTrue)
                    .help(builder.text("cli-status-global-help")?),
            ))
    }

    pub(super) fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Scope> {
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
}

#[cfg(test)]
#[path = "status_test.rs"]
mod status_test;

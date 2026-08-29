//! `destroy`のcommand-line adapter。

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::boundary::terminal::PromptCapability;
use crate::commands::destroy::Args;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

use super::super::help::Builder;
use super::super::project_arg;

pub(super) struct Destroy;

impl Destroy {
    pub(super) fn spec(builder: &Builder) -> Result<ClapCommand> {
        Ok(builder
            .positional("destroy", "cli-destroy-about")?
            .arg(
                Arg::new("project")
                    .value_name(project_arg::PROJECT_VALUE_NAME)
                    .help(builder.text("cli-destroy-project-help")?),
            )
            .arg(
                Arg::new("force")
                    .long("force")
                    .short('f')
                    .action(ArgAction::SetTrue)
                    .help(builder.text("cli-destroy-force-help")?),
            ))
    }

    pub(super) fn parse(matches: &ArgMatches, prompt: PromptCapability) -> Result<Args> {
        let force = matches.get_flag("force");
        let project = matches.get_one::<String>("project");
        match project {
            Some(value) => Ok(Args {
                project: Some(ProjectId::parse(value)?),
                force,
            }),
            None if force => fail(
                ErrorId::ProjectArgumentRequired,
                msg!(
                    "error-project-argument-required",
                    subcommand = "sbxm destroy --force"
                ),
            ),
            None => {
                project_arg::require_prompt_capability(prompt, "sbxm destroy")?;
                Ok(Args {
                    project: None,
                    force,
                })
            }
        }
    }
}

#[cfg(test)]
#[path = "destroy_test.rs"]
mod destroy_test;

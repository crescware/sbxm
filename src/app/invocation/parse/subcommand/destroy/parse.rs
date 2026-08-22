use clap::ArgMatches;

use crate::app::invocation::Interactivity;
use crate::commands::destroy::Args;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

pub(crate) fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Args> {
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
            super::super::super::project_arg::require_prompt_capability(
                interactivity,
                "sbxm destroy",
            )?;
            Ok(Args {
                project: None,
                force,
            })
        }
    }
}

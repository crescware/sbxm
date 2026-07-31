use clap::ArgMatches;

use crate::cli::Interactivity;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

use super::Args;

pub fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Args> {
    let force = matches.get_flag("force");
    let project = matches.get_one::<String>("project");
    match project {
        Some(value) => Ok(Args {
            project: Some(ProjectId::parse(value)?),
            force,
        }),
        None if force => {
            // force modeはTTYかどうかにかかわらず完全指定を必須とする。
            fail(
                ErrorId::ProjectArgumentRequired,
                msg!(
                    "error-project-argument-required",
                    subcommand = "sbxm destroy --force"
                ),
            )
        }
        None => {
            crate::cli::project_arg::require_prompt_capability(interactivity, "sbxm destroy")?;
            Ok(Args {
                project: None,
                force,
            })
        }
    }
}

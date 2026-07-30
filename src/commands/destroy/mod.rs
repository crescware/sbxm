//! `sbxm destroy`。

mod exec;
pub mod print;
pub mod run;

pub use exec::exec;

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::Interactivity;
use crate::cli::project_arg::PROJECT_VALUE_NAME;
use crate::error::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

/// `destroy`の対象と mode。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// force modeはTTYかどうかにかかわらずproject引数の完全指定を必須とする。
    pub project: Option<ProjectId>,
    pub force: bool,
}

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder
        .positional("destroy", "cli-destroy-about")?
        .arg(
            Arg::new("project")
                .value_name(PROJECT_VALUE_NAME)
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

pub fn args(matches: &ArgMatches, interactivity: Interactivity) -> Result<Args> {
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

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;

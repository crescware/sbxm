//! `apply`のcommand-line adapter。

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::app::invocation::Interactivity;
use crate::commands::apply::Args;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use super::super::help::Builder;
use super::super::project_arg;

pub(super) struct Apply;

impl Apply {
    pub(super) fn spec(builder: &Builder) -> Result<ClapCommand> {
        Ok(builder
            .positional("apply", "cli-apply-about")?
            .arg(
                Arg::new("project")
                    .value_name(project_arg::PROJECT_VALUE_NAME)
                    .help(builder.text("cli-apply-project-help")?),
            )
            .arg(
                Arg::new("files")
                    .long("files")
                    .action(ArgAction::SetTrue)
                    .help(builder.text("cli-apply-files-help")?),
            )
            .arg(
                Arg::new("worktrees")
                    .long("worktrees")
                    .short('t')
                    .value_name("N")
                    .value_parser(clap::value_parser!(u32))
                    .help(builder.text("cli-apply-worktrees-help")?),
            ))
    }

    pub(super) fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Args> {
        let files = matches.get_flag("files");
        let worktrees = matches.get_one::<u32>("worktrees").copied();
        if !files && worktrees.is_none() {
            return fail(
                ErrorId::ApplyScopeRequired,
                msg!("error-apply-scope-required"),
            );
        }
        Ok(Args {
            project: project_arg::optional_project(matches, interactivity, "sbxm apply")?,
            files,
            worktrees,
        })
    }
}

#[cfg(test)]
#[path = "apply_test.rs"]
mod apply_test;

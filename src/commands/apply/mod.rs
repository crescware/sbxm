//! `sbxm apply`。

mod exec;
#[cfg(test)]
mod fake;
pub mod print;
pub mod run;

pub use exec::exec;

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::project_arg::{PROJECT_VALUE_NAME, required_project};
use crate::error::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

/// `apply`の引数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub project: ProjectId,
    /// global configが宣言するfileを再配置する。
    pub files: bool,
    /// managed worktreeの目標本数。
    pub worktrees: Option<u32>,
}

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder
        .positional("apply", "cli-apply-about")?
        .arg(
            Arg::new("project")
                .required(true)
                .value_name(PROJECT_VALUE_NAME)
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
                .value_name("N")
                .value_parser(clap::value_parser!(u32))
                .help(builder.text("cli-apply-worktrees-help")?),
        ))
}

/// 省略した対象へは触れないため、何も指定しない実行は何をするか決まらない。
pub fn args(matches: &ArgMatches) -> Result<Args> {
    let files = matches.get_flag("files");
    let worktrees = matches.get_one::<u32>("worktrees").copied();
    if !files && worktrees.is_none() {
        return fail(
            ErrorId::ApplyScopeRequired,
            msg!("error-apply-scope-required"),
        );
    }
    Ok(Args {
        project: required_project(matches)?,
        files,
        worktrees,
    })
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;

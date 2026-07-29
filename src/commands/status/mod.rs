//! `sbxm status`。
//!
//! host環境を診断するglobal scopeと、1案件を診断するproject scopeを持つ。どちらの
//! scopeもread-onlyで、状態を変えない。

mod exec;
pub mod global;
mod print;
pub mod project;

pub use exec::exec;

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::project_arg::PROJECT_VALUE_NAME;
use crate::error::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

/// `status`のscope。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Global,
    Project(ProjectId),
}

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder
        .positional("status", "cli-status-about")?
        .arg(
            Arg::new("project")
                .value_name(PROJECT_VALUE_NAME)
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

pub fn args(matches: &ArgMatches) -> Result<Scope> {
    let global = matches.get_flag("global");
    let project = matches.get_one::<String>("project");
    match (global, project) {
        (true, None) => Ok(Scope::Global),
        (false, Some(value)) => Ok(Scope::Project(ProjectId::parse(value)?)),
        _ => fail(
            ErrorId::StatusScopeRequired,
            msg!("error-status-scope-required"),
        ),
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;

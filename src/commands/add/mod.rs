//! `sbxm add`。

mod exec;
mod host_clone;
pub mod print;
pub mod run;

pub use exec::exec;

use clap::{Arg, ArgMatches, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::project_arg::{CLONE_URL_VALUE_NAME, required_clone_url};
use crate::error::{ErrorId, Result, fail};
use crate::metadata::{MAX_WORKTREES, MIN_WORKTREES};
use crate::msg;
use crate::repository::RepositoryIdentity;

/// `add`の目標構成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub repository: RepositoryIdentity,
    pub worktrees: Option<u32>,
    pub detach: Option<String>,
}

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder
        .positional("add", "cli-add-about")?
        .arg(
            Arg::new("repository")
                .required(true)
                .value_name(CLONE_URL_VALUE_NAME)
                .help(builder.text("cli-add-repository-help")?),
        )
        .arg(
            Arg::new("worktrees")
                .long("worktrees")
                .value_name("N")
                .help(builder.text("cli-add-worktrees-help")?),
        )
        .arg(
            Arg::new("detach")
                .long("detach")
                .value_name("BRANCH")
                .help(builder.text("cli-add-detach-help")?),
        ))
}

pub fn args(matches: &ArgMatches) -> Result<Args> {
    let repository = required_clone_url(matches)?;
    let detach = matches.get_one::<String>("detach").cloned();

    let worktrees = match matches.get_one::<String>("worktrees") {
        Some(raw) => {
            let parsed: Option<u32> = raw.parse().ok();
            match parsed {
                Some(value) if (MIN_WORKTREES..=MAX_WORKTREES).contains(&value) => Some(value),
                _ => {
                    return fail(
                        ErrorId::WorktreesOutOfRange,
                        msg!(
                            "error-worktrees-out-of-range",
                            value = raw,
                            minimum = MIN_WORKTREES,
                            maximum = MAX_WORKTREES
                        ),
                    );
                }
            }
        }
        None => None,
    };

    // 2個以上のmanaged worktreeは、起点branchの明示を必須とする。
    if worktrees.is_some_and(|value| value >= 2) && detach.is_none() {
        return fail(
            ErrorId::WorktreesRequireDetach,
            msg!("error-worktrees-require-detach"),
        );
    }

    Ok(Args {
        repository,
        worktrees,
        detach,
    })
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;

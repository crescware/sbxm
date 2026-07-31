use clap::ArgMatches;

use crate::cli::project_arg::required_project;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use super::Args;

/// 省略した対象へは触れないため、何も指定しない実行は何をするか決まらない。
pub fn parse(matches: &ArgMatches) -> Result<Args> {
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

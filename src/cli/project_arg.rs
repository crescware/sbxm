//! 案件を指す位置引数。
//!
//! 対象の省略を許すcommandは、非TTYで省略された場合に外部状態を読む前に終了する。

use clap::ArgMatches;

use crate::error::{Error, ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;
use crate::repository::RepositoryIdentity;

use super::Interactivity;

/// 案件を指す位置引数のvalue name。
///
/// clapはrequiredなvalueを`<>`、optionalなvalueを`[]`で囲む。どちらの表示でも読めるよう、
/// value name自体には囲み記号を含めない。
pub const PROJECT_VALUE_NAME: &str = "owner/repository";

/// 登録対象を指す位置引数のvalue name。
pub const CLONE_URL_VALUE_NAME: &str = "github-clone-url";

/// GitHubが表示するclone URLを解釈する。
pub fn required_clone_url(matches: &ArgMatches) -> Result<RepositoryIdentity> {
    let value = matches.get_one::<String>("repository").ok_or_else(|| {
        Error::new(
            ErrorId::MissingRequiredArgument,
            msg!(
                "error-missing-required-argument",
                argument = "<github-clone-url>"
            ),
        )
    })?;
    RepositoryIdentity::parse_clone_url(value)
}

pub fn required_project(matches: &ArgMatches) -> Result<ProjectId> {
    let value = matches.get_one::<String>("project").ok_or_else(|| {
        Error::new(
            ErrorId::MissingRequiredArgument,
            msg!(
                "error-missing-required-argument",
                argument = "<owner/repository>"
            ),
        )
    })?;
    ProjectId::parse(value)
}

pub fn optional_project(
    matches: &ArgMatches,
    interactivity: Interactivity,
    command: &str,
) -> Result<Option<ProjectId>> {
    match matches.get_one::<String>("project") {
        Some(value) => Ok(Some(ProjectId::parse(value)?)),
        None => {
            require_prompt_capability(interactivity, command)?;
            Ok(None)
        }
    }
}

/// 非TTYで対象を省略した場合は、外部状態を読む前に終了する。
pub fn require_prompt_capability(interactivity: Interactivity, command: &str) -> Result<()> {
    if interactivity.can_prompt() {
        return Ok(());
    }
    fail(
        ErrorId::ProjectArgumentRequired,
        msg!("error-project-argument-required", subcommand = command),
    )
}

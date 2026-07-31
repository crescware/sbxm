use clap::ArgMatches;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::repository::RepositoryIdentity;

/// `GitHubが表示するclone` URLを解釈する。
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

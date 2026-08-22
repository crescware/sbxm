use clap::ArgMatches;

use crate::commands::add::Args;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result, fail};
use crate::metadata::{GitIdentity, MAX_WORKTREES, MIN_WORKTREES, validate_git_identity_value};
use crate::msg;

pub(crate) fn parse(matches: &ArgMatches) -> Result<Args> {
    let repository = super::super::super::project_arg::required_clone_url(matches)?;
    let detach = matches.get_one::<String>("detach").cloned();
    let git_identity = declared_git_identity(matches)?;

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
        git_identity,
    })
}

fn declared_git_identity(matches: &ArgMatches) -> Result<Option<GitIdentity>> {
    let user_name = matches.get_one::<String>("git-user-name").cloned();
    let user_email = matches.get_one::<String>("git-user-email").cloned();

    match (user_name, user_email) {
        (None, None) => Ok(None),
        (Some(user_name), Some(user_email)) => {
            check_declared_value("--git-user-name", &user_name)?;
            check_declared_value("--git-user-email", &user_email)?;
            Ok(Some(GitIdentity {
                user_name,
                user_email,
            }))
        }
        (declared_name, _) => {
            let missing = if declared_name.is_some() {
                "--git-user-email"
            } else {
                "--git-user-name"
            };
            fail(
                ErrorId::GitIdentityIncomplete,
                msg!("error-git-identity-incomplete", missing = missing),
            )
        }
    }
}

fn check_declared_value(field: &str, value: &str) -> Result<()> {
    match validate_git_identity_value(value) {
        Ok(()) => Ok(()),
        Err(reason) => Err(Error::single(
            Diagnostic::new(ErrorId::InvalidValue, msg!("error-git-identity-invalid"))
                .fact(Fact::field(field))
                .fact(Fact::reason(reason)),
        )),
    }
}

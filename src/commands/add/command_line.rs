//! `add`のparser非依存command-line解釈。

use crate::boundary::command_line::{
    ArgumentSyntax, Arguments, Builder, CommandLayout, CommandSyntax,
};
use crate::commands::command_line_values::CommandLineValues;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result, fail};
use crate::metadata::{GitIdentity, MAX_WORKTREES, MIN_WORKTREES, validate_git_identity_value};
use crate::msg;

use super::Args;

pub(crate) struct CommandLine;

impl CommandLine {
    pub(crate) fn syntax(builder: &Builder) -> Result<CommandSyntax> {
        Ok(builder
            .command("add", "cli-add-about", CommandLayout::Positional)?
            .arg(
                ArgumentSyntax::value("repository", builder.text("cli-add-repository-help")?)
                    .value_name(CommandLineValues::CLONE_URL_VALUE_NAME)
                    .required(),
            )
            .arg(
                ArgumentSyntax::value("worktrees", builder.text("cli-add-worktrees-help")?)
                    .long("worktrees")
                    .short('t')
                    .value_name("N"),
            )
            .arg(
                ArgumentSyntax::value("detach", builder.text("cli-add-detach-help")?)
                    .long("detach")
                    .value_name("BRANCH"),
            )
            .arg(
                ArgumentSyntax::value("git-user-name", builder.text("cli-add-git-user-name-help")?)
                    .long("git-user-name")
                    .value_name("NAME"),
            )
            .arg(
                ArgumentSyntax::value(
                    "git-user-email",
                    builder.text("cli-add-git-user-email-help")?,
                )
                .long("git-user-email")
                .value_name("EMAIL"),
            ))
    }

    pub(crate) fn interpret(arguments: &Arguments) -> Result<Args> {
        let repository = CommandLineValues::required_clone_url(arguments)?;
        let detach = arguments.value("detach").map(str::to_owned);
        let git_identity = declared_git_identity(arguments)?;

        let worktrees = match arguments.value("worktrees") {
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
}

fn declared_git_identity(arguments: &Arguments) -> Result<Option<GitIdentity>> {
    let user_name = arguments.value("git-user-name").map(str::to_owned);
    let user_email = arguments.value("git-user-email").map(str::to_owned);

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

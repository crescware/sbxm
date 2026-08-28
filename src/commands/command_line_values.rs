use crate::boundary::command_line::Arguments;
use crate::boundary::terminal::PromptCapability;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;
use crate::repository::RepositoryIdentity;

/// application commandが中立なcommand-line valuesを解釈する共通部品。
pub(crate) struct CommandLineValues;

impl CommandLineValues {
    pub(crate) const CLONE_URL_VALUE_NAME: &'static str = "github-clone-url";
    pub(crate) const PROJECT_VALUE_NAME: &'static str = "project-id";

    pub(crate) fn required_clone_url(arguments: &Arguments) -> Result<RepositoryIdentity> {
        let value = arguments.value("repository").ok_or_else(|| {
            Error::new(
                ErrorId::MissingRequiredArgument,
                msg!(
                    "error-missing-required-argument",
                    argument = format!("<{}>", Self::CLONE_URL_VALUE_NAME)
                ),
            )
        })?;
        RepositoryIdentity::parse_clone_url(value)
    }

    pub(crate) fn optional_project(
        arguments: &Arguments,
        prompt: PromptCapability,
        command: &str,
    ) -> Result<Option<ProjectId>> {
        if let Some(value) = arguments.value("project") {
            Ok(Some(ProjectId::parse(value)?))
        } else {
            Self::require_prompt_capability(prompt, command)?;
            Ok(None)
        }
    }

    pub(crate) fn require_prompt_capability(prompt: PromptCapability, command: &str) -> Result<()> {
        if prompt.can_prompt() {
            return Ok(());
        }
        fail(
            ErrorId::ProjectArgumentRequired,
            msg!("error-project-argument-required", subcommand = command),
        )
    }

    pub(crate) fn optional_u32(
        arguments: &Arguments,
        id: &'static str,
        option: &'static str,
    ) -> Result<Option<u32>> {
        arguments
            .value(id)
            .map(|value| {
                value.parse::<u32>().map_err(|_| {
                    Error::single(Diagnostic::new(
                        ErrorId::InvalidValue,
                        msg!("error-invalid-value", argument = option, value = value),
                    ))
                })
            })
            .transpose()
    }
}

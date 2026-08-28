//! `destroy`のparser非依存command-line解釈。

use crate::boundary::command_line::{ArgumentSyntax, Arguments, Builder, CommandSyntax};
use crate::boundary::terminal::PromptCapability;
use crate::commands::command_line_values::CommandLineValues;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

use super::Args;

pub(crate) struct CommandLine;

impl CommandLine {
    pub(crate) fn syntax(builder: &Builder) -> Result<CommandSyntax> {
        Ok(builder
            .command("destroy", "cli-destroy-about")?
            .arg(
                ArgumentSyntax::value("project", builder.text("cli-destroy-project-help")?)
                    .value_name(CommandLineValues::PROJECT_VALUE_NAME),
            )
            .arg(
                ArgumentSyntax::flag("force", builder.text("cli-destroy-force-help")?)
                    .long("force")
                    .short('f'),
            ))
    }

    pub(crate) fn interpret(arguments: &Arguments, prompt: PromptCapability) -> Result<Args> {
        let force = arguments.flag("force");
        let project = arguments.value("project");
        match project {
            Some(value) => Ok(Args {
                project: Some(ProjectId::parse(value)?),
                force,
            }),
            None if force => fail(
                ErrorId::ProjectArgumentRequired,
                msg!(
                    "error-project-argument-required",
                    subcommand = "sbxm destroy --force"
                ),
            ),
            None => {
                CommandLineValues::require_prompt_capability(prompt, "sbxm destroy")?;
                Ok(Args {
                    project: None,
                    force,
                })
            }
        }
    }
}

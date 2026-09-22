//! `guide`のparser非依存command-line解釈。

use crate::boundary::command_line::{ArgumentSyntax, Arguments, Builder, CommandSyntax};
use crate::boundary::terminal::PromptCapability;
use crate::commands::command_line_values::CommandLineValues;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

use super::{Args, Topic};

pub(crate) struct CommandLine;

impl CommandLine {
    pub(crate) fn syntax(builder: &Builder) -> Result<CommandSyntax> {
        Ok(builder
            .command("guide", "cli-guide-about")?
            .arg(
                ArgumentSyntax::value("topic", builder.text("cli-guide-topic-help")?)
                    .value_name("topic"),
            )
            .arg(
                ArgumentSyntax::value("project", builder.text("cli-guide-project-help")?)
                    .value_name(CommandLineValues::PROJECT_VALUE_NAME),
            ))
    }

    pub(crate) fn interpret(arguments: &Arguments, prompt: PromptCapability) -> Result<Args> {
        let topic = arguments.value("topic").map(Topic::parse).transpose()?;
        if topic.is_none() && !prompt.can_prompt() {
            return fail(
                ErrorId::MissingRequiredArgument,
                msg!("error-missing-required-argument", argument = "<topic>"),
            );
        }

        let project = match arguments.value("project") {
            Some(value) => Some(ProjectId::parse(value)?),
            None if topic.is_none() => None,
            None => CommandLineValues::optional_project(
                arguments,
                prompt,
                "sbxm guide credential-rotation",
            )?,
        };

        Ok(Args { topic, project })
    }
}

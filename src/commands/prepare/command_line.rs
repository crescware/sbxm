//! `prepare`のparser非依存command-line解釈。

use crate::boundary::command_line::{ArgumentSyntax, Arguments, Builder, CommandSyntax};
use crate::boundary::terminal::PromptCapability;
use crate::commands::command_line_values::CommandLineValues;
use crate::diagnostics::Result;
use crate::project::ProjectId;

pub(crate) struct CommandLine;

impl CommandLine {
    pub(crate) fn syntax(builder: &Builder) -> Result<CommandSyntax> {
        Ok(builder.command("prepare", "cli-prepare-about")?.arg(
            ArgumentSyntax::value("project", builder.text("cli-prepare-project-help")?)
                .value_name(CommandLineValues::PROJECT_VALUE_NAME),
        ))
    }

    pub(crate) fn interpret(
        arguments: &Arguments,
        prompt: PromptCapability,
    ) -> Result<Option<ProjectId>> {
        CommandLineValues::optional_project(arguments, prompt, "sbxm prepare")
    }
}

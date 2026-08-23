//! `open`のparser非依存command-line解釈。

use crate::boundary::command_line::{
    ArgumentSyntax, Arguments, Builder, CommandLayout, CommandSyntax,
};
use crate::boundary::terminal::PromptCapability;
use crate::commands::command_line_values::CommandLineValues;
use crate::diagnostics::Result;

use super::Args;

pub(crate) struct CommandLine;

impl CommandLine {
    pub(crate) fn syntax(builder: &Builder) -> Result<CommandSyntax> {
        Ok(builder
            .command("open", "cli-open-about", CommandLayout::Positional)?
            .arg(
                ArgumentSyntax::value("project", builder.text("cli-open-project-help")?)
                    .value_name(CommandLineValues::PROJECT_VALUE_NAME),
            )
            .arg(
                ArgumentSyntax::value("index", builder.text("cli-open-index-help")?)
                    .long("index")
                    .short('i')
                    .value_name("N"),
            ))
    }

    pub(crate) fn interpret(arguments: &Arguments, prompt: PromptCapability) -> Result<Args> {
        Ok(Args {
            project: CommandLineValues::optional_project(arguments, prompt, "sbxm open")?,
            index: CommandLineValues::optional_u32(arguments, "index", "--index")?,
        })
    }
}

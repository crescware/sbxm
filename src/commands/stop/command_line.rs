//! `stop`のparser非依存command-line解釈。

use crate::boundary::command_line::{
    ArgumentSyntax, Arguments, Builder, CommandLayout, CommandSyntax,
};
use crate::boundary::terminal::PromptCapability;
use crate::commands::command_line_values::CommandLineValues;
use crate::diagnostics::Result;
use crate::project::ProjectId;

pub(crate) struct CommandLine;

impl CommandLine {
    pub(crate) fn syntax(builder: &Builder) -> Result<CommandSyntax> {
        Ok(builder
            .command("stop", "cli-stop-about", CommandLayout::Positional)?
            .arg(
                ArgumentSyntax::append("project", builder.text("cli-stop-project-help")?)
                    .value_name(CommandLineValues::PROJECT_VALUE_NAME),
            ))
    }

    pub(crate) fn interpret(
        arguments: &Arguments,
        prompt: PromptCapability,
    ) -> Result<Vec<ProjectId>> {
        let values: Vec<String> = arguments.values("project").map(str::to_owned).collect();
        if values.is_empty() {
            CommandLineValues::require_prompt_capability(prompt, "sbxm stop")?;
            return Ok(Vec::new());
        }
        values
            .into_iter()
            .map(|value| ProjectId::parse(&value))
            .collect()
    }
}

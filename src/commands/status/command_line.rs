//! `status`のparser非依存command-line解釈。

use crate::boundary::command_line::{ArgumentSyntax, Arguments, Builder, CommandSyntax};
use crate::boundary::terminal::PromptCapability;
use crate::commands::command_line_values::CommandLineValues;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::project::ProjectId;

use super::Scope;

pub(crate) struct CommandLine;

impl CommandLine {
    pub(crate) fn syntax(builder: &Builder) -> Result<CommandSyntax> {
        Ok(builder
            .command("status", "cli-status-about")?
            .arg(
                ArgumentSyntax::value("project", builder.text("cli-status-project-help")?)
                    .value_name(CommandLineValues::PROJECT_VALUE_NAME),
            )
            .arg(
                ArgumentSyntax::flag("global", builder.text("cli-status-global-help")?)
                    .long("global")
                    .short('g'),
            ))
    }

    pub(crate) fn interpret(arguments: &Arguments, prompt: PromptCapability) -> Result<Scope> {
        let global = arguments.flag("global");
        let project = arguments.value("project");
        match (global, project) {
            (true, None) => Ok(Scope::Global),
            (false, Some(value)) => Ok(Scope::Project(ProjectId::parse(value)?)),
            (false, None) if prompt.can_prompt() => Ok(Scope::Prompt),
            _ => fail(
                ErrorId::StatusScopeRequired,
                msg!("error-status-scope-required"),
            ),
        }
    }
}

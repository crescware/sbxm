//! `apply`のparser非依存command-line解釈。

use crate::boundary::command_line::{ArgumentSyntax, Arguments, Builder, CommandSyntax};
use crate::boundary::terminal::PromptCapability;
use crate::commands::command_line_values::CommandLineValues;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use super::Args;

pub(crate) struct CommandLine;

impl CommandLine {
    pub(crate) fn syntax(builder: &Builder) -> Result<CommandSyntax> {
        Ok(builder
            .command("apply", "cli-apply-about")?
            .arg(
                ArgumentSyntax::value("project", builder.text("cli-apply-project-help")?)
                    .value_name(CommandLineValues::PROJECT_VALUE_NAME),
            )
            .arg(ArgumentSyntax::flag("files", builder.text("cli-apply-files-help")?).long("files"))
            .arg(
                ArgumentSyntax::value("worktrees", builder.text("cli-apply-worktrees-help")?)
                    .long("worktrees")
                    .short('t')
                    .value_name("N"),
            ))
    }

    pub(crate) fn interpret(arguments: &Arguments, prompt: PromptCapability) -> Result<Args> {
        let files = arguments.flag("files");
        let worktrees = CommandLineValues::optional_u32(arguments, "worktrees", "--worktrees")?;
        if !files && worktrees.is_none() {
            return fail(
                ErrorId::ApplyScopeRequired,
                msg!("error-apply-scope-required"),
            );
        }
        Ok(Args {
            project: CommandLineValues::optional_project(arguments, prompt, "sbxm apply")?,
            files,
            worktrees,
        })
    }
}

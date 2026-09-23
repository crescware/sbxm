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
            .arg(ArgumentSyntax::flag("force", builder.text("cli-apply-force-help")?).long("force"))
            .arg(ArgumentSyntax::flag("all", builder.text("cli-apply-all-help")?).long("all"))
            .arg(
                ArgumentSyntax::value("worktrees", builder.text("cli-apply-worktrees-help")?)
                    .long("worktrees")
                    .short('t')
                    .value_name("N"),
            ))
    }

    pub(crate) fn interpret(arguments: &Arguments, prompt: PromptCapability) -> Result<Args> {
        let files = arguments.flag("files");
        let force = arguments.flag("force");
        let worktrees = CommandLineValues::optional_u32(arguments, "worktrees", "--worktrees")?;
        if !files && worktrees.is_none() {
            return fail(
                ErrorId::ApplyScopeRequired,
                msg!("error-apply-scope-required"),
            );
        }
        // 置き換えを許す対象が無いまま受け付けると、何を許したのかが読み手に分からない。
        if force && !files {
            return fail(
                ErrorId::ApplyForceWithoutFiles,
                msg!("error-apply-force-without-files"),
            );
        }
        let all = arguments.flag("all");
        if all {
            // 全案件へ一律に適用できるのは宣言fileだけである。worktreeの本数は案件ごとに違う。
            let conflicting = match (arguments.value("project"), worktrees) {
                (Some(_), _) => Some(format!(
                    "<{}>, --all",
                    CommandLineValues::PROJECT_VALUE_NAME
                )),
                (None, Some(_)) => Some("--all, --worktrees".to_string()),
                (None, None) => None,
            };
            if let Some(arguments) = conflicting {
                return fail(
                    ErrorId::ConflictingArguments,
                    msg!("error-conflicting-arguments", arguments = arguments),
                );
            }
            return Ok(Args {
                project: None,
                all,
                files,
                force,
                worktrees,
            });
        }
        Ok(Args {
            project: CommandLineValues::optional_project(arguments, prompt, "sbxm apply")?,
            all,
            files,
            force,
            worktrees,
        })
    }
}

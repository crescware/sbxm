//! `files`のparser非依存command-line解釈。

use std::path::PathBuf;

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
            .command("files", "cli-files-about")?
            .arg(
                ArgumentSyntax::value("action", builder.text("cli-files-action-help")?)
                    .value_name("add|ls|rm|pull"),
            )
            .arg(
                ArgumentSyntax::value("path", builder.text("cli-files-path-help")?)
                    .value_name("path"),
            )
            .arg(
                ArgumentSyntax::value("project", builder.text("cli-files-project-help")?)
                    .value_name(CommandLineValues::PROJECT_VALUE_NAME),
            )
            .arg(
                ArgumentSyntax::value("dest", builder.text("cli-files-dest-help")?)
                    .long("dest")
                    .value_name("destination"),
            ))
    }

    pub(crate) fn interpret(arguments: &Arguments, prompt: PromptCapability) -> Result<Args> {
        let path = arguments.value("path");
        let destination = arguments.value("dest");
        let Some(action) = arguments.value("action") else {
            return fail(
                ErrorId::MissingRequiredArgument,
                msg!(
                    "error-missing-required-argument",
                    argument = "<add|ls|rm|pull>"
                ),
            );
        };
        // 案件を選ぶのはSandboxから取り出すときだけである。
        if let Some(project) = arguments.value("project")
            && action != "pull"
        {
            return fail(
                ErrorId::UnknownArgument,
                msg!("error-unknown-argument", argument = project),
            );
        }
        if action == "pull" {
            return pull(arguments, path, destination, prompt);
        }
        match (action, path, destination) {
            ("add", Some(path), destination) => Ok(Args::Add {
                source: PathBuf::from(path),
                destination: destination.map(str::to_string),
            }),
            ("add", None, _) => fail(
                ErrorId::MissingRequiredArgument,
                msg!("error-missing-required-argument", argument = "<path>"),
            ),
            ("ls", None, None) => Ok(Args::Ls),
            ("rm", Some(path), None) => Ok(Args::Rm {
                destination: path.to_string(),
            }),
            ("rm", None, None) => fail(
                ErrorId::MissingRequiredArgument,
                msg!(
                    "error-missing-required-argument",
                    argument = "<destination>"
                ),
            ),
            // 配置先を選べるのは足すときだけである。並べる・外す実行では意味を持たない。
            ("ls" | "rm", _, Some(_)) => fail(
                ErrorId::ConflictingArguments,
                msg!(
                    "error-conflicting-arguments",
                    arguments = format!("{action}, --dest")
                ),
            ),
            ("ls", Some(path), None) => fail(
                ErrorId::UnknownArgument,
                msg!("error-unknown-argument", argument = path),
            ),
            (other, _, _) => fail(
                ErrorId::InvalidValue,
                msg!(
                    "error-invalid-value",
                    argument = "<add|ls|rm|pull>",
                    value = other
                ),
            ),
        }
    }
}

/// `files pull <destination> [<project-id>]`。
fn pull(
    arguments: &Arguments,
    path: Option<&str>,
    destination: Option<&str>,
    prompt: PromptCapability,
) -> Result<Args> {
    if destination.is_some() {
        return fail(
            ErrorId::ConflictingArguments,
            msg!("error-conflicting-arguments", arguments = "pull, --dest"),
        );
    }
    let Some(path) = path else {
        return fail(
            ErrorId::MissingRequiredArgument,
            msg!(
                "error-missing-required-argument",
                argument = "<destination>"
            ),
        );
    };
    Ok(Args::Pull {
        destination: path.to_string(),
        project: CommandLineValues::optional_project(arguments, prompt, "sbxm files pull")?,
    })
}

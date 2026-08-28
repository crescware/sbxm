use crate::boundary::command_line::{Builder, ParsedCommand, ParsedCommandLine};
use crate::boundary::terminal::PromptCapability;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::project::ProjectId;

use super::{add, apply, destroy, ls, open, prepare, rebuild, status, stop};

/// 実行するcommand。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// parserが組み立てたhelp本文をstdoutへ提示する。
    Help(String),
    /// parserが組み立てたversion行をstdoutへ提示する。
    Version(String),
    Add(crate::commands::add::Args),
    Apply(crate::commands::apply::Args),
    Prepare(Option<ProjectId>),
    Rebuild(Option<ProjectId>),
    Open(crate::commands::open::Args),
    Stop(Vec<ProjectId>),
    Ls,
    Status(crate::commands::status::Scope),
    Destroy(crate::commands::destroy::Args),
}

impl Command {
    pub(crate) fn syntax(
        builder: &Builder,
    ) -> Result<Vec<crate::boundary::command_line::CommandSyntax>> {
        Ok(vec![
            add::CommandLineParser::syntax(builder)?,
            apply::CommandLineParser::syntax(builder)?,
            prepare::CommandLineParser::syntax(builder)?,
            rebuild::CommandLineParser::syntax(builder)?,
            open::CommandLineParser::syntax(builder)?,
            stop::CommandLineParser::syntax(builder)?,
            ls::CommandLineParser::syntax(builder)?,
            status::CommandLineParser::syntax(builder)?,
            destroy::CommandLineParser::syntax(builder)?,
        ])
    }

    pub(crate) fn interpret(
        parsed: ParsedCommandLine,
        prompt: PromptCapability,
    ) -> Result<Command> {
        match parsed {
            ParsedCommandLine::Help(text) => Ok(Command::Help(text)),
            ParsedCommandLine::Version(text) => Ok(Command::Version(text)),
            ParsedCommandLine::Command(ParsedCommand { name, arguments }) => match name.as_str() {
                "add" => Ok(Command::Add(add::CommandLineParser::interpret(&arguments)?)),
                "apply" => Ok(Command::Apply(apply::CommandLineParser::interpret(
                    &arguments, prompt,
                )?)),
                "prepare" => Ok(Command::Prepare(prepare::CommandLineParser::interpret(
                    &arguments, prompt,
                )?)),
                "rebuild" => Ok(Command::Rebuild(rebuild::CommandLineParser::interpret(
                    &arguments, prompt,
                )?)),
                "open" => Ok(Command::Open(open::CommandLineParser::interpret(
                    &arguments, prompt,
                )?)),
                "stop" => Ok(Command::Stop(stop::CommandLineParser::interpret(
                    &arguments, prompt,
                )?)),
                "ls" => Ok(Command::Ls),
                "status" => Ok(Command::Status(status::CommandLineParser::interpret(
                    &arguments, prompt,
                )?)),
                "destroy" => Ok(Command::Destroy(destroy::CommandLineParser::interpret(
                    &arguments, prompt,
                )?)),
                other => Err(Error::new(
                    ErrorId::UnknownSubcommand,
                    msg!("error-unknown-subcommand", subcommand = other),
                )),
            },
        }
    }
}

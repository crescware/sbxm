use crate::boundary::command_line::{Builder, parse as parse_command_line};
use crate::boundary::terminal::PromptCapability;
use crate::commands::Command;
use crate::diagnostics::Result;
use crate::i18n::Catalog;

/// localeとprompt能力が確定したargvをapplication commandへ解釈する。
pub(crate) fn parse(
    argv: &[String],
    catalog: &Catalog,
    prompt: PromptCapability,
) -> Result<Command> {
    let builder = Builder::new(catalog)?;
    let syntax = Command::syntax(&builder)?;
    let parsed = parse_command_line(argv, catalog, &syntax)?;
    Command::interpret(parsed, prompt)
}

use crate::boundary::command_line::{Arguments, CommandSyntax, ParsedCommand, ParsedCommandLine};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::i18n::Catalog;
use crate::msg;

use super::{build_parser, diagnostics};

pub(crate) fn parse(
    argv: &[String],
    catalog: &Catalog,
    syntaxes: &[CommandSyntax],
) -> Result<ParsedCommandLine> {
    let parser = build_parser(catalog, syntaxes)?;
    let matches = match parser.try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => return diagnostics::interpret(&error),
    };

    let (name, sub) = matches
        .subcommand()
        .ok_or_else(|| Error::new(ErrorId::MissingSubcommand, msg!("error-missing-subcommand")))?;
    let syntax = syntaxes
        .iter()
        .find(|syntax| syntax.name == name)
        .ok_or_else(|| {
            Error::new(
                ErrorId::UnknownSubcommand,
                msg!("error-unknown-subcommand", subcommand = name),
            )
        })?;
    let mut arguments = Arguments::default();
    for argument in &syntax.arguments {
        match argument.action {
            super::super::ArgumentAction::Flag => {
                arguments.insert_flag(argument.id, sub.get_flag(argument.id));
            }
            super::super::ArgumentAction::Value => {
                if let Some(value) = sub.get_one::<String>(argument.id) {
                    arguments.insert_value(argument.id, value.clone());
                }
            }
            super::super::ArgumentAction::Append => {
                if let Some(values) = sub.get_many::<String>(argument.id) {
                    for value in values {
                        arguments.insert_value(argument.id, value.clone());
                    }
                }
            }
        }
    }
    Ok(ParsedCommandLine::Command(ParsedCommand {
        name: name.to_owned(),
        arguments,
    }))
}

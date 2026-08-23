use crate::commands::Command;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::i18n::Catalog;
use crate::msg;

use super::super::Interactivity;
use super::subcommand::Subcommand;
use super::{build_parser, diagnostics};

/// localeと対話可能性が確定したargvをparseする。
pub(crate) fn parse(
    argv: &[String],
    catalog: &Catalog,
    interactivity: Interactivity,
) -> Result<Command> {
    let parser = build_parser::build_parser(catalog)?;
    let matches = match parser.try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => return diagnostics::interpret(&error),
    };

    let (name, sub) = matches
        .subcommand()
        .ok_or_else(|| Error::new(ErrorId::MissingSubcommand, msg!("error-missing-subcommand")))?;
    Subcommand::from_matches(name, sub, interactivity)
}

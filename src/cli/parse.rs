use crate::commands::{self};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::i18n::Catalog;
use crate::msg;

use super::{Interactivity, Outcome, build_command};

/// localeを決めたcatalogでargvをparseする。
pub fn parse(argv: &[String], catalog: &Catalog, interactivity: Interactivity) -> Result<Outcome> {
    let command = build_command(catalog)?;
    let matches = match command.try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => return crate::cli::diagnostics::interpret(&error),
    };
    // parserは`subcommand_required`であり、subcommandの無いargvはclapが先に拒否する。
    // ここへ届いたmatchesは必ずsubcommandを持つため、この拒否は`Option`が要求する
    // 分岐として在る。利用者が見るのは、同じ`ErrorId`を持つclap側の診断である。
    let (name, sub) = matches
        .subcommand()
        .ok_or_else(|| Error::new(ErrorId::MissingSubcommand, msg!("error-missing-subcommand")))?;
    Ok(Outcome::Run(commands::from_matches(
        name,
        sub,
        interactivity,
    )?))
}

use clap::ArgMatches;

use crate::app::invocation::Interactivity;
use crate::commands::Command;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

/// parserが受け入れたsubcommand名を、実行要求のvariantへ変換する。
pub(crate) fn from_matches(
    name: &str,
    matches: &ArgMatches,
    interactivity: Interactivity,
) -> Result<Command> {
    match name {
        "add" => Ok(Command::Add(super::add::parse(matches)?)),
        "apply" => Ok(Command::Apply(super::apply::parse(matches, interactivity)?)),
        "prepare" => Ok(Command::Prepare(super::prepare::parse(
            matches,
            interactivity,
        )?)),
        "rebuild" => Ok(Command::Rebuild(super::rebuild::parse(
            matches,
            interactivity,
        )?)),
        "open" => Ok(Command::Open(super::open::parse(matches, interactivity)?)),
        "stop" => Ok(Command::Stop(super::stop::parse(matches, interactivity)?)),
        "ls" => Ok(Command::Ls),
        "status" => Ok(Command::Status(super::status::parse(
            matches,
            interactivity,
        )?)),
        "destroy" => Ok(Command::Destroy(super::destroy::parse(
            matches,
            interactivity,
        )?)),
        other => fail(
            ErrorId::UnknownSubcommand,
            msg!("error-unknown-subcommand", subcommand = other),
        ),
    }
}

#[cfg(test)]
#[path = "from_matches_test.rs"]
mod from_matches_test;

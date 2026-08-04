use clap::ArgMatches;

use crate::cli::Interactivity;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use super::Command;

/// 解析済みのsubcommandを、実行するcommandへ組み立てる。
pub fn from_matches(
    name: &str,
    matches: &ArgMatches,
    interactivity: Interactivity,
) -> Result<Command> {
    match name {
        "add" => Ok(Command::Add(crate::commands::add::parse(matches)?)),
        "apply" => Ok(Command::Apply(crate::commands::apply::parse(matches)?)),
        "prepare" => Ok(Command::Prepare(crate::commands::prepare::parse(
            matches,
            interactivity,
        )?)),
        "rebuild" => Ok(Command::Rebuild(crate::commands::rebuild::parse(matches)?)),
        "open" => Ok(Command::Open(crate::commands::open::parse(
            matches,
            interactivity,
        )?)),
        "stop" => Ok(Command::Stop(crate::commands::stop::parse(
            matches,
            interactivity,
        )?)),
        "ls" => Ok(Command::Ls),
        "status" => Ok(Command::Status(crate::commands::status::parse(matches)?)),
        "destroy" => Ok(Command::Destroy(crate::commands::destroy::parse(
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

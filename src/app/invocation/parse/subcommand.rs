//! subcommandのcommand-line adapter。
//!
//! helpに並ぶ順と、parserが受け入れた名前から実行要求への対応を、このmoduleが1箇所で
//! 持つ。個々のadapterは自分の引数だけを知り、ほかのsubcommandの存在を知らない。

mod add;
mod apply;
mod destroy;
mod ls;
mod open;
mod prepare;
mod rebuild;
mod status;
mod stop;

use clap::{ArgMatches, Command as ClapCommand};

use crate::commands::Command;
use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use super::super::Interactivity;
use super::help::Builder;
use add::Add;
use apply::Apply;
use destroy::Destroy;
use ls::Ls;
use open::Open;
use prepare::Prepare;
use rebuild::Rebuild;
use status::Status;
use stop::Stop;

/// 全subcommandの一覧。
pub(super) struct Subcommand;

impl Subcommand {
    /// helpとusageに並ぶ順で、全subcommandのspecを組み立てる。
    pub(super) fn specs(builder: &Builder) -> Result<Vec<ClapCommand>> {
        Ok(vec![
            Add::spec(builder)?,
            Apply::spec(builder)?,
            Prepare::spec(builder)?,
            Rebuild::spec(builder)?,
            Open::spec(builder)?,
            Stop::spec(builder)?,
            Ls::spec(builder)?,
            Status::spec(builder)?,
            Destroy::spec(builder)?,
        ])
    }

    /// parserが受け入れたsubcommand名を、実行要求のvariantへ変換する。
    pub(super) fn from_matches(
        name: &str,
        matches: &ArgMatches,
        interactivity: Interactivity,
    ) -> Result<Command> {
        match name {
            "add" => Ok(Command::Add(Add::parse(matches)?)),
            "apply" => Ok(Command::Apply(Apply::parse(matches, interactivity)?)),
            "prepare" => Ok(Command::Prepare(Prepare::parse(matches, interactivity)?)),
            "rebuild" => Ok(Command::Rebuild(Rebuild::parse(matches, interactivity)?)),
            "open" => Ok(Command::Open(Open::parse(matches, interactivity)?)),
            "stop" => Ok(Command::Stop(Stop::parse(matches, interactivity)?)),
            "ls" => Ok(Command::Ls),
            "status" => Ok(Command::Status(Status::parse(matches, interactivity)?)),
            "destroy" => Ok(Command::Destroy(Destroy::parse(matches, interactivity)?)),
            other => fail(
                ErrorId::UnknownSubcommand,
                msg!("error-unknown-subcommand", subcommand = other),
            ),
        }
    }
}

#[cfg(test)]
#[path = "subcommand_test.rs"]
mod subcommand_test;

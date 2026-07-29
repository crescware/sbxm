//! 公開command。
//!
//! 1 commandが1 directoryを持ち、そのdirectoryが引数、実行、出力を持つ。1 commandからしか
//! 呼ばれないものはそのdirectoryへ置き、commandをまたぐものは`crate::support`が持つ。
//!
//! 本moduleはcommandの一覧そのものとし、command固有の知識を持たない。

pub mod add;
pub mod apply;
pub mod destroy;
pub mod init;
pub mod ls;
pub mod open;
pub mod prepare;
pub mod rebuild;
pub mod status;
pub mod stop;

mod context;

pub use context::{Context, report};

use clap::{ArgMatches, Command as ClapCommand};

use crate::cli::{Builder, Interactivity};
use crate::error::{ErrorId, ExitCode, Result, fail};
use crate::msg;
use crate::project::ProjectId;

/// 実行するcommand。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init(init::Mode),
    Add(add::Args),
    Apply(apply::Args),
    Prepare(ProjectId),
    Rebuild(ProjectId),
    Open(Option<ProjectId>),
    Stop(Vec<ProjectId>),
    Ls,
    Status(status::Scope),
    Destroy(destroy::Args),
}

/// parserへ登録するsubcommand。この並び順がhelpの並び順になる。
pub fn specs(builder: &Builder) -> Result<Vec<ClapCommand>> {
    Ok(vec![
        init::spec(builder)?,
        add::spec(builder)?,
        apply::spec(builder)?,
        prepare::spec(builder)?,
        rebuild::spec(builder)?,
        open::spec(builder)?,
        stop::spec(builder)?,
        ls::spec(builder)?,
        status::spec(builder)?,
        destroy::spec(builder)?,
    ])
}

/// 解析済みのsubcommandを、実行するcommandへ組み立てる。
pub fn from_matches(
    name: &str,
    matches: &ArgMatches,
    interactivity: Interactivity,
) -> Result<Command> {
    match name {
        "init" => Ok(Command::Init(init::args(matches)?)),
        "add" => Ok(Command::Add(add::args(matches)?)),
        "apply" => Ok(Command::Apply(apply::args(matches)?)),
        "prepare" => Ok(Command::Prepare(prepare::args(matches)?)),
        "rebuild" => Ok(Command::Rebuild(rebuild::args(matches)?)),
        "open" => Ok(Command::Open(open::args(matches, interactivity)?)),
        "stop" => Ok(Command::Stop(stop::args(matches, interactivity)?)),
        "ls" => Ok(Command::Ls),
        "status" => Ok(Command::Status(status::args(matches)?)),
        "destroy" => Ok(Command::Destroy(destroy::args(matches, interactivity)?)),
        other => fail(
            ErrorId::UnknownSubcommand,
            msg!("error-unknown-subcommand", subcommand = other),
        ),
    }
}

/// commandを実行し、結果を表示してexit codeを返す。
pub fn dispatch(command: &Command, context: &Context) -> ExitCode {
    match command {
        Command::Init(mode) => init::exec(mode, context),
        Command::Add(args) => add::exec(args, context),
        Command::Apply(args) => apply::exec(args, context),
        Command::Prepare(project) => prepare::exec(project, context),
        Command::Rebuild(project) => rebuild::exec(project, context),
        Command::Open(project) => open::exec(project.as_ref(), context),
        Command::Stop(projects) => stop::exec(projects, context),
        Command::Ls => ls::exec(context),
        Command::Status(scope) => status::exec(scope, context),
        Command::Destroy(args) => destroy::exec(args, context),
    }
}

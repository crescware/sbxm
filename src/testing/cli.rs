//! CLI parseのtestが共有するargvの組み立て。

use crate::cli::{Interactivity, Outcome, parse};
use crate::commands::Command;
use crate::error::Result;
use crate::i18n::{Catalog, Locale};

pub fn argv(arguments: &[&str]) -> Vec<String> {
    std::iter::once("sbxm".to_string())
        .chain(arguments.iter().map(|value| value.to_string()))
        .collect()
}

pub fn tty() -> Interactivity {
    Interactivity {
        stdin_is_tty: true,
        stderr_is_tty: true,
    }
}

pub fn non_tty() -> Interactivity {
    Interactivity {
        stdin_is_tty: false,
        stderr_is_tty: false,
    }
}

/// 正本localeでargvをparseする。
pub fn parse_argv(arguments: &[&str], interactivity: Interactivity) -> Result<Outcome> {
    let catalog = Catalog::new(Locale::En);
    parse(&argv(arguments), &catalog, interactivity)
}

/// parseが成功し、commandを返すことを前提に取り出す。
pub fn command(arguments: &[&str], interactivity: Interactivity) -> Command {
    match parse_argv(arguments, interactivity).expect("the arguments parse") {
        Outcome::Run(command) => command,
        other => panic!("expected a command, got {other:?}"),
    }
}

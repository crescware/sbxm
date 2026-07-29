//! CLI parse。
//!
//! CLI parser libraryの自動help・自動終了へlocale決定を委ねず、選択したlocaleで
//! help、usage、parse errorを生成する。validationは次の順で行う。
//!
//! 1. syntaxとoption関係
//! 2. `--lang`
//! 3. command固有の引数
//! 4. config load
//! 5. project解決
//! 6. 外部command
//! 7. mutation
//!
//! 本moduleは1から3までを担当し、config、filesystem、外部状態には触れない。
//! 3のcommand固有部分は`crate::commands`の各commandが持つ。

mod diagnostics;
pub mod help;
mod interactivity;
mod lang;
pub mod project_arg;

pub use help::Builder;
pub use interactivity::Interactivity;
pub use lang::{PeekedLang, invalid_lang_error, peek_lang};

use clap::{Arg, ArgAction, Command as ClapCommand};

use crate::commands::{self, Command};
use crate::error::{Error, ErrorId, Result};
use crate::i18n::Catalog;
use crate::msg;

/// parse結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// helpをstdoutへ出してexit code `0`。
    Help(String),
    /// version文字列をstdoutへ出してexit code `0`。
    Version(String),
    Run(Command),
}

/// localeを決めたcatalogでargvをparseする。
pub fn parse(argv: &[String], catalog: &Catalog, interactivity: Interactivity) -> Result<Outcome> {
    let command = build_command(catalog)?;
    let matches = match command.try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => return diagnostics::interpret(error),
    };
    let (name, sub) = matches
        .subcommand()
        .ok_or_else(|| Error::new(ErrorId::MissingSubcommand, msg!("error-missing-subcommand")))?;
    Ok(Outcome::Run(commands::from_matches(
        name,
        sub,
        interactivity,
    )?))
}

/// `--version`が表示する文字列。
pub fn version_line() -> String {
    format!("sbxm {}", env!("CARGO_PKG_VERSION"))
}

/// FTLからhelp textを組み立てたparserを作る。
fn build_command(catalog: &Catalog) -> Result<ClapCommand> {
    let builder = Builder::new(catalog)?;

    let mut command = ClapCommand::new("sbxm")
        .about(builder.text("cli-about")?)
        .version(env!("CARGO_PKG_VERSION"))
        .help_template(builder.root_template())
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(false)
        .arg(lang::arg(&builder)?)
        .arg(builder.help_flag())
        .arg(
            Arg::new("version")
                .long("version")
                .short('V')
                .action(ArgAction::Version)
                .display_order(1001)
                .help(builder.text("cli-version-help")?),
        );

    // 並び順がhelpの並び順になる。
    for subcommand in commands::specs(&builder)? {
        command = command.subcommand(subcommand);
    }
    Ok(command)
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;

//! `sbxm init`。

mod exec;
pub mod run;

pub use exec::exec;

use clap::{Arg, ArgMatches, Command as ClapCommand};

use crate::cli::Builder;
use crate::error::{ErrorId, Result, fail};
use crate::msg;

/// `init`の2 mode。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// 3 optionを1つも指定しない。
    Interactive,
    /// 3 optionをすべて指定する。
    Options {
        base_path: String,
        git_user_name: String,
        git_user_email: String,
    },
}

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder
        .leaf("init", "cli-init-about")?
        .arg(
            Arg::new("base-path")
                .long("base-path")
                .value_name("PATH")
                .help(builder.text("cli-init-base-path-help")?),
        )
        .arg(
            Arg::new("git-user-name")
                .long("git-user-name")
                .value_name("NAME")
                .help(builder.text("cli-init-git-user-name-help")?),
        )
        .arg(
            Arg::new("git-user-email")
                .long("git-user-email")
                .value_name("EMAIL")
                .help(builder.text("cli-init-git-user-email-help")?),
        ))
}

pub fn args(matches: &ArgMatches) -> Result<Mode> {
    let base_path = matches.get_one::<String>("base-path").cloned();
    let git_user_name = matches.get_one::<String>("git-user-name").cloned();
    let git_user_email = matches.get_one::<String>("git-user-email").cloned();

    let provided = [&base_path, &git_user_name, &git_user_email]
        .iter()
        .filter(|value| value.is_some())
        .count();

    match provided {
        0 => Ok(Mode::Interactive),
        3 => Ok(Mode::Options {
            base_path: base_path.expect("checked above"),
            git_user_name: git_user_name.expect("checked above"),
            git_user_email: git_user_email.expect("checked above"),
        }),
        _ => {
            // configやfilesystemを読む前に、不足optionを表示して終了する。
            let mut missing = Vec::new();
            if base_path.is_none() {
                missing.push("--base-path");
            }
            if git_user_name.is_none() {
                missing.push("--git-user-name");
            }
            if git_user_email.is_none() {
                missing.push("--git-user-email");
            }
            fail(
                ErrorId::InitIncompleteOptions,
                msg!(
                    "error-init-incomplete-options",
                    missing = missing.join(", ")
                ),
            )
        }
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;

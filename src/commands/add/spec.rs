use clap::{Arg, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::project_arg::CLONE_URL_VALUE_NAME;
use crate::diagnostics::Result;

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder
        .positional("add", "cli-add-about")?
        .arg(
            Arg::new("repository")
                .required(true)
                .value_name(CLONE_URL_VALUE_NAME)
                .help(builder.text("cli-add-repository-help")?),
        )
        .arg(
            Arg::new("worktrees")
                .long("worktrees")
                .short('t')
                .value_name("N")
                .help(builder.text("cli-add-worktrees-help")?),
        )
        .arg(
            Arg::new("detach")
                .long("detach")
                .value_name("BRANCH")
                .help(builder.text("cli-add-detach-help")?),
        )
        .arg(
            Arg::new("git-user-name")
                .long("git-user-name")
                .value_name("NAME")
                .help(builder.text("cli-add-git-user-name-help")?),
        )
        .arg(
            Arg::new("git-user-email")
                .long("git-user-email")
                .value_name("EMAIL")
                .help(builder.text("cli-add-git-user-email-help")?),
        ))
}

use clap::{Arg, ArgAction, Command as ClapCommand};

use super::super::super::super::help::Builder;
use super::super::super::project_arg::PROJECT_VALUE_NAME;
use crate::diagnostics::Result;

pub(crate) fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder
        .positional("apply", "cli-apply-about")?
        .arg(
            Arg::new("project")
                .value_name(PROJECT_VALUE_NAME)
                .help(builder.text("cli-apply-project-help")?),
        )
        .arg(
            Arg::new("files")
                .long("files")
                .action(ArgAction::SetTrue)
                .help(builder.text("cli-apply-files-help")?),
        )
        .arg(
            Arg::new("worktrees")
                .long("worktrees")
                .short('t')
                .value_name("N")
                .value_parser(clap::value_parser!(u32))
                .help(builder.text("cli-apply-worktrees-help")?),
        ))
}

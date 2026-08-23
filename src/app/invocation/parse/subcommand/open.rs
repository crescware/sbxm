//! `open`のcommand-line adapter。

use clap::{Arg, ArgMatches, Command as ClapCommand};

use crate::app::invocation::Interactivity;
use crate::commands::open::Args;
use crate::diagnostics::Result;

use super::super::help::Builder;
use super::super::project_arg;

pub(super) struct Open;

impl Open {
    pub(super) fn spec(builder: &Builder) -> Result<ClapCommand> {
        Ok(builder
            .positional("open", "cli-open-about")?
            .arg(
                Arg::new("project")
                    .value_name(project_arg::PROJECT_VALUE_NAME)
                    .help(builder.text("cli-open-project-help")?),
            )
            .arg(
                Arg::new("index")
                    .long("index")
                    .short('i')
                    .value_name("N")
                    .value_parser(clap::value_parser!(u32))
                    .help(builder.text("cli-open-index-help")?),
            ))
    }

    pub(super) fn parse(matches: &ArgMatches, interactivity: Interactivity) -> Result<Args> {
        Ok(Args {
            project: project_arg::optional_project(matches, interactivity, "sbxm open")?,
            index: matches.get_one::<u32>("index").copied(),
        })
    }
}

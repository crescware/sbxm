//! `prepare`のcommand-line adapter。

use clap::{Arg, ArgMatches, Command as ClapCommand};

use crate::app::invocation::Interactivity;
use crate::diagnostics::Result;
use crate::project::ProjectId;

use super::super::help::Builder;
use super::super::project_arg;

pub(super) struct Prepare;

impl Prepare {
    pub(super) fn spec(builder: &Builder) -> Result<ClapCommand> {
        Ok(builder.positional("prepare", "cli-prepare-about")?.arg(
            Arg::new("project")
                .value_name(project_arg::PROJECT_VALUE_NAME)
                .help(builder.text("cli-prepare-project-help")?),
        ))
    }

    pub(super) fn parse(
        matches: &ArgMatches,
        interactivity: Interactivity,
    ) -> Result<Option<ProjectId>> {
        project_arg::optional_project(matches, interactivity, "sbxm prepare")
    }
}

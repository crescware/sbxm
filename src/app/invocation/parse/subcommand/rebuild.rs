//! `rebuild`のcommand-line adapter。

use clap::{Arg, ArgMatches, Command as ClapCommand};

use crate::app::invocation::Interactivity;
use crate::diagnostics::Result;
use crate::project::ProjectId;

use super::super::help::Builder;
use super::super::project_arg;

pub(super) struct Rebuild;

impl Rebuild {
    pub(super) fn spec(builder: &Builder) -> Result<ClapCommand> {
        Ok(builder.positional("rebuild", "cli-rebuild-about")?.arg(
            Arg::new("project")
                .value_name(project_arg::PROJECT_VALUE_NAME)
                .help(builder.text("cli-rebuild-project-help")?),
        ))
    }

    pub(super) fn parse(
        matches: &ArgMatches,
        interactivity: Interactivity,
    ) -> Result<Option<ProjectId>> {
        project_arg::optional_project(matches, interactivity, "sbxm rebuild")
    }
}

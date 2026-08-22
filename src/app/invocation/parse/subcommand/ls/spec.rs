use clap::Command as ClapCommand;

use super::super::super::super::help::Builder;
use crate::diagnostics::Result;

pub(crate) fn spec(builder: &Builder) -> Result<ClapCommand> {
    builder.leaf("ls", "cli-ls-about")
}

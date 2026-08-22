use clap::Command as ClapCommand;

use crate::cli::Builder;
use crate::diagnostics::Result;

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    builder.leaf("ls", "cli-ls-about")
}

//! `sbxm ls`。

mod exec;
mod print;
pub mod run;

pub use exec::exec;

use clap::Command as ClapCommand;

use crate::cli::Builder;
use crate::error::Result;

pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    builder.leaf("ls", "cli-ls-about")
}

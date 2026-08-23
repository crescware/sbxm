//! `ls`のcommand-line adapter。

use clap::Command as ClapCommand;

use crate::diagnostics::Result;

use super::super::help::Builder;

pub(super) struct Ls;

impl Ls {
    pub(super) fn spec(builder: &Builder) -> Result<ClapCommand> {
        builder.leaf("ls", "cli-ls-about")
    }
}

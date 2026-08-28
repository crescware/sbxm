//! `ls`のparser非依存command-line解釈。

use crate::boundary::command_line::{Builder, CommandSyntax};
use crate::diagnostics::Result;

pub(crate) struct CommandLine;

impl CommandLine {
    pub(crate) fn syntax(builder: &Builder) -> Result<CommandSyntax> {
        builder.command("ls", "cli-ls-about")
    }
}

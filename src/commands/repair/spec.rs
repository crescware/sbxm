use clap::{Arg, Command as ClapCommand};

use crate::cli::Builder;
use crate::cli::project_arg::PROJECT_VALUE_NAME;
use crate::diagnostics::Result;

/// `sbxm repair`のCLI surface。
pub fn spec(builder: &Builder) -> Result<ClapCommand> {
    Ok(builder.positional("repair", "cli-repair-about")?.arg(
        Arg::new("project")
            .value_name(PROJECT_VALUE_NAME)
            .help(builder.text("cli-repair-project-help")?),
    ))
}

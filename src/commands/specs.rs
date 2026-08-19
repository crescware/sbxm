use clap::Command as ClapCommand;

use crate::cli::Builder;
use crate::diagnostics::Result;

/// parserへ登録するsubcommand。この並び順がhelpの並び順になる。
pub fn specs(builder: &Builder) -> Result<Vec<ClapCommand>> {
    Ok(vec![
        crate::commands::add::spec(builder)?,
        crate::commands::apply::spec(builder)?,
        crate::commands::prepare::spec(builder)?,
        crate::commands::repair::spec(builder)?,
        crate::commands::rebuild::spec(builder)?,
        crate::commands::open::spec(builder)?,
        crate::commands::stop::spec(builder)?,
        crate::commands::ls::spec(builder)?,
        crate::commands::status::spec(builder)?,
        crate::commands::destroy::spec(builder)?,
    ])
}

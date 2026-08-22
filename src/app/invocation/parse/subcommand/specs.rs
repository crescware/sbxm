use clap::Command as ClapCommand;

use crate::app::invocation::help::Builder;
use crate::diagnostics::Result;

/// 全subcommandの表示順を所有する。
pub(crate) fn specs(builder: &Builder) -> Result<Vec<ClapCommand>> {
    Ok(vec![
        super::add::spec(builder)?,
        super::apply::spec(builder)?,
        super::prepare::spec(builder)?,
        super::rebuild::spec(builder)?,
        super::open::spec(builder)?,
        super::stop::spec(builder)?,
        super::ls::spec(builder)?,
        super::status::spec(builder)?,
        super::destroy::spec(builder)?,
    ])
}

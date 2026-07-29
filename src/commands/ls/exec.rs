//! `ls`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::support::sandbox;

use super::super::{Context, report};
use super::print;

pub fn exec(context: &Context) -> ExitCode {
    let (config, catalog) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    match super::run::run(
        &config,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
    ) {
        Ok(listing) => {
            print::listing(&catalog, &listing);
            ExitCode::Success
        }
        Err(error) => report(&catalog, &error),
    }
}

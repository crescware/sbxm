//! `prepare`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::project::ProjectId;
use crate::support::sandbox;

use super::super::{Context, report};
use super::print;

pub fn exec(project: &ProjectId, context: &Context) -> ExitCode {
    let (config, catalog) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    match super::run::run(
        &config,
        project,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
    ) {
        Ok(output) => {
            print::output(&catalog, &output);
            ExitCode::Success
        }
        Err(error) => report(&catalog, &error),
    }
}

//! `apply`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::support::sandbox;

use super::super::{Context, report};
use super::run::Scope;
use super::{Args, print};

pub fn exec(args: &Args, context: &Context) -> ExitCode {
    let (config, catalog) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    let scope = Scope {
        files: args.files,
        worktrees: args.worktrees,
    };
    match super::run::run(
        &config,
        &args.project,
        scope,
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

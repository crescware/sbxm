//! `add`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;

use super::super::{Context, report};
use super::run::AddRequest;
use super::{Args, print};

pub fn exec(args: &Args, context: &Context) -> ExitCode {
    let (config, catalog) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    let request = AddRequest {
        project: args.project.clone(),
        worktrees: args.worktrees,
        detach: args.detach.clone(),
    };
    match super::run::run(&config, &request, &RealHost) {
        Ok(output) => {
            print::output(&catalog, &output);
            ExitCode::Success
        }
        Err(error) => report(&catalog, &error),
    }
}

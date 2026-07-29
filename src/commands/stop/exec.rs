//! `stop`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::project::ProjectId;
use crate::support::select::TerminalProjectPrompt;
use crate::support::{inventory, sandbox};

use super::super::{Context, report};
use super::print;

pub fn exec(projects: &[ProjectId], context: &Context) -> ExitCode {
    let (config, catalog) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    let mut prompt = TerminalProjectPrompt {
        heading: "select-stop-heading",
        locale: catalog.locale(),
    };
    match super::run::run(
        &config,
        projects,
        &RealHost,
        &mut prompt,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        inventory::Poll::default(),
    ) {
        Ok(stopped) => print::report(&catalog, &stopped),
        Err(error) => report(&catalog, &error),
    }
}

//! `status`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::project::ProjectId;
use crate::support::sandbox;

use super::super::{Context, report};
use super::{Scope, print};

pub fn exec(scope: &Scope, context: &Context) -> ExitCode {
    match scope {
        Scope::Global => global(context),
        Scope::Project(project) => project_scope(project, context),
    }
}

fn global(context: &Context) -> ExitCode {
    let catalog = match context.tolerant_catalog() {
        Ok(catalog) => catalog,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    let status = super::global::diagnose(context.location, &RealHost);
    print::global(&catalog, &status)
}

fn project_scope(project: &ProjectId, context: &Context) -> ExitCode {
    let (config, catalog) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    match super::project::diagnose(
        &config,
        project,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
    ) {
        Ok(status) => print::project(&catalog, &status),
        Err(error) => report(&catalog, &error),
    }
}

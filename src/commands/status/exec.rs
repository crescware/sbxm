//! `status`の実行。

use crate::command::HostEnvironment;
use crate::design::Ui;
use crate::diagnostics::ExitCode;
use crate::project::ProjectId;
use crate::support::sandbox;

use super::{
    super::{Context, report},
    Scope, print,
};

pub fn exec(scope: &Scope, context: &Context, ui: &mut Ui, host: &dyn HostEnvironment) -> ExitCode {
    match scope {
        Scope::Global => global(context, ui, host),
        Scope::Project(project) => project_scope(project, context, ui, host),
    }
}

fn global(context: &Context, ui: &mut Ui, host: &dyn HostEnvironment) -> ExitCode {
    let locale = match context.tolerant_locale() {
        Ok(locale) => locale,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    let status = super::global::diagnose(context.location, host);
    print::global(ui, &status)
}

fn project_scope(
    project: &ProjectId,
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
) -> ExitCode {
    let (_config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    match super::project::diagnose(
        context.location,
        project,
        host,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
    ) {
        Ok(status) => print::project(ui, &status),
        Err(error) => report(ui, &error),
    }
}

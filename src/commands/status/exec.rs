//! `status`の実行。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::project::ProjectId;
use crate::support::sandbox;
use crate::ui::Ui;

use super::super::{Context, report};
use super::{Scope, print};

pub fn exec(scope: &Scope, context: &Context, ui: &mut Ui) -> ExitCode {
    match scope {
        Scope::Global => global(context, ui),
        Scope::Project(project) => project_scope(project, context, ui),
    }
}

fn global(context: &Context, ui: &mut Ui) -> ExitCode {
    let locale = match context.tolerant_locale() {
        Ok(locale) => locale,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    let status = super::global::diagnose(context.location, &RealHost);
    print::global(ui, &status)
}

fn project_scope(project: &ProjectId, context: &Context, ui: &mut Ui) -> ExitCode {
    let (config, locale) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    match super::project::diagnose(
        &config,
        project,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
    ) {
        Ok(status) => print::project(ui, &status),
        Err(error) => report(ui, &error),
    }
}

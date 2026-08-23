//! `status`の実行。

use crate::boundary::host::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::{ExitCode, Result};
use crate::msg;
use crate::project::ProjectId;
use crate::support::select;

use super::{
    super::{Context, report},
    Scope, print,
};

pub fn exec(
    scope: &Scope,
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    match scope {
        Scope::Global => global(context, ui, host),
        Scope::Project(project) => project_scope(project, context, ui, host),
        Scope::Prompt => prompt_scope(context, ui, host, prompt),
    }
}

fn prompt_scope(
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    let locale = match context.tolerant_locale() {
        Ok(locale) => locale,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    prompt.set_locale(locale);
    let scope = match select_scope(context.location, prompt) {
        Ok(scope) => scope,
        Err(error) => return report(ui, &error),
    };
    match scope {
        Scope::Global => global(context, ui, host),
        Scope::Project(project) => project_scope(&project, context, ui, host),
        Scope::Prompt => unreachable!("select_scope never returns an unresolved prompt"),
    }
}

fn select_scope(
    location: &crate::config::ConfigLocation,
    prompt: &mut dyn select::ProjectPrompt,
) -> Result<Scope> {
    let candidates = select::candidates(location)?;
    let mut labels = Vec::with_capacity(candidates.len() + 1);
    labels.push("global".to_owned());
    labels.extend(candidates.iter().map(select::Candidate::display_id));
    let index = prompt.select_one(&msg!("select-status-heading"), &labels)?;
    match index {
        0 => Ok(Scope::Global),
        project_index => {
            let candidate = candidates
                .get(project_index - 1)
                .ok_or_else(|| select::unresolved(index, labels.len()))?;
            Ok(Scope::Project(ProjectId::parse(&candidate.display_id())?))
        }
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
    match super::project::diagnose(context.location, project, host, context.workspace_root) {
        Ok(status) => print::project(ui, &status),
        Err(error) => report(ui, &error),
    }
}

#[cfg(test)]
#[path = "exec_test.rs"]
mod exec_test;

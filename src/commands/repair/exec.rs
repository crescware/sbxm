use crate::command::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;

use super::{
    super::{Context, report},
    print, run,
};

pub fn exec(
    project: Option<&crate::project::ProjectId>,
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    let (config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    prompt.set_locale(locale);

    let prepared = match run::prepare(
        context.location,
        &config,
        project,
        host,
        prompt,
        context.workspace_root,
    ) {
        Ok(prepared) => prepared,
        Err(error) => return report(ui, &error),
    };

    let initial_view = prepared.view();
    ui.stdout(&print::document(&initial_view, locale));
    match prepared {
        run::Prepared::Fresh { .. } | run::Prepared::Healthy { .. } => ExitCode::Success,
        run::Prepared::Repairable(plan) => {
            let target = plan.target_generation.clone();
            match run::execute(*plan, &config, host, context.workspace_root, ui) {
                Ok(output) => {
                    for warning in &output.warnings {
                        ui.warning(warning);
                    }
                    ui.stdout(&print::document(
                        &run::repaired_view(&output, &target),
                        locale,
                    ));
                    ExitCode::Success
                }
                Err(error) => report(ui, &error),
            }
        }
    }
}

#[cfg(test)]
#[path = "exec_test.rs"]
mod exec_test;

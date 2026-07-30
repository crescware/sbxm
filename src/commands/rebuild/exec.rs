//! `rebuild`の実行と出力。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::hash::short_hex;
use crate::msg;
use crate::project::ProjectId;
use crate::support::{inventory, sandbox};
use crate::ui::{Document, Ui};

use super::super::{Context, report};

pub fn exec(project: &ProjectId, context: &Context, ui: &mut Ui) -> ExitCode {
    let (config, locale) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    let output = match super::run::run(
        &config,
        project,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        inventory::Poll::default(),
        ui,
    ) {
        Ok(output) => output,
        Err(error) => return report(ui, &error),
    };

    // warningは結果を隠さない。成功と注意の両方を別blockとして残す。
    for warning in &output.warnings {
        ui.warning(warning);
    }
    let summary = msg!(
        "rebuild-applied",
        project = output.project,
        sandbox = output.sandbox,
        generation = short_hex(&output.applied)
    );
    ui.stdout(&Document::new().summary(summary));
    ExitCode::Success
}

use crate::design::Ui;
use crate::diagnostics::ExitCode;

use crate::commands::rebuild::RebuildOutput;

use super::document;

/// `rebuild`の結果を表示する。
///
/// warningは結果を隠さない。成功と注意の両方を別blockとして残し、注意が付いた実行も
/// 成功として終える。
pub fn report(ui: &mut Ui, output: &RebuildOutput) -> ExitCode {
    for warning in &output.warnings {
        ui.warning(warning);
    }
    ui.stdout(&document(output));
    ExitCode::Success
}

#[cfg(test)]
#[path = "report_test.rs"]
mod report_test;

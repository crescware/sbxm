use crate::design::Ui;
use crate::diagnostics::ExitCode;

use crate::commands::sync::SyncOutput;

use super::document;

/// `sync`の結果を表示する。
///
/// gitが断ったrefがあれば、`git push`や`git fetch`と同じくexit code `1`とする。scriptが、
/// 同期しきれなかったことに気付けるようにする。Sandboxのbranchが遅れているだけのものは
/// 数えない。
pub fn report(ui: &mut Ui, output: &SyncOutput) -> ExitCode {
    ui.stdout(&document(output, ui.locale()));
    if output.refused_any() {
        ExitCode::Failure
    } else {
        ExitCode::Success
    }
}

#[cfg(test)]
#[path = "report_test.rs"]
mod report_test;

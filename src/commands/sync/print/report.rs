use crate::design::Ui;
use crate::diagnostics::ExitCode;

use crate::commands::sync::SyncOutput;

use super::document;

/// `sync`の結果を表示する。
///
/// gitが断り、hostのbranchやtagがそのまま残ったものがあれば、`git push`と同じくexit
/// code `1`とする。scriptが、同期しきれなかったことに気付けるようにする。Sandboxの
/// branchが遅れているだけのものは数えない。
pub fn report(ui: &mut Ui, output: &SyncOutput) -> ExitCode {
    ui.stdout(&document(output, ui.locale()));
    if output.left_as_it_was() {
        ExitCode::Failure
    } else {
        ExitCode::Success
    }
}

#[cfg(test)]
#[path = "report_test.rs"]
mod report_test;

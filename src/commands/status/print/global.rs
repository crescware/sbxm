use crate::design::{Document, Ui};
use crate::diagnostics::ExitCode;

use crate::commands::status::global::GlobalStatus;

use super::global_document;

/// global scopeの`status`。
pub fn global(ui: &mut Ui, status: &GlobalStatus) -> ExitCode {
    ui.stdout(&global_document(status, ui.locale()));

    for warning in &status.warnings {
        ui.warning(warning);
    }
    for diagnostic in &status.diagnostics {
        ui.stderr(&Document::new().diagnostic(diagnostic.clone()));
    }

    if status.is_healthy() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}

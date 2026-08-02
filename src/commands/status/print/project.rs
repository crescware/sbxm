use crate::design::{Document, Ui};
use crate::diagnostics::ExitCode;

use crate::commands::status::project::ProjectStatus;

use super::project_document;

/// project scopeの`status`。
pub fn project(ui: &mut Ui, status: &ProjectStatus) -> ExitCode {
    ui.stdout(&project_document(status, ui.locale()));

    for diagnostic in &status.diagnostics {
        ui.stderr(&Document::new().diagnostic(diagnostic.clone()));
    }
    if status.is_healthy() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}

#[cfg(test)]
#[path = "project_test.rs"]
mod project_test;

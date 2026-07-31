use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use crate::design::Remediation;

use super::{Item, Value, WorktreeRow};

/// 診断結果。
#[derive(Debug, Clone)]
pub struct ProjectStatus {
    pub project: String,
    pub items: Vec<Item>,
    pub worktrees: Vec<WorktreeRow>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ProjectStatus {
    pub fn is_healthy(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub(crate) fn push(&mut self, item: &'static str, value: Value) {
        self.items.push(Item { item, value });
    }

    /// global環境を読めなかったため観測できなかったことを、別commandの案内とともに残す。
    pub(crate) fn global_scope_failure(&mut self, error: &Error) {
        self.diagnostics.extend(error.diagnostics().iter().cloned());
        self.diagnostics.push(
            Diagnostic::new(
                ErrorId::GlobalScopeUnobservable,
                msg!("error-global-scope-unobservable"),
            )
            .remediation(
                Remediation::text(msg!("remediation-run-global-status"))
                    .try_run("sbxm status --global"),
            ),
        );
    }
}

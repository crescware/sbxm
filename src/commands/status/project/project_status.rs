use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use crate::design::Remediation;
use crate::support::disk::DiskObservation;
use crate::support::provisioning::NextAction;

use super::{Item, Value, WorktreeRow};

/// 診断結果。
#[derive(Debug, Clone)]
pub struct ProjectStatus {
    pub project: String,
    pub items: Vec<Item>,
    pub worktrees: Vec<WorktreeRow>,
    /// root filesystemの使用量。`items`とは別に持ち、payloadを持てない`Value`には
    /// 押し込まない。
    pub disk: DiskObservation,
    pub diagnostics: Vec<Diagnostic>,
    /// 観測から導いた、今すぐ実行できる1手。相反するcommandを並べないため、多くとも
    /// 1件しか持たない。
    pub next: Option<NextAction>,
}

impl ProjectStatus {
    /// 目標構成に達しており、次の保守操作も要らないか。
    ///
    /// 診断が1件でもあれば失敗とする。加えて、中断した初回構築や未完了の世代交代の
    /// ように、案件がまだ目標構成へ達していないことを示す1手が要る場合も失敗とする。
    /// Dockerfileの変更は破損ではないため、案内しても成功のまま終える。
    pub fn is_healthy(&self) -> bool {
        self.diagnostics.is_empty() && !self.next.is_some_and(NextAction::is_blocking)
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

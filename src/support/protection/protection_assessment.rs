use crate::project::SandboxName;

use super::{ConfirmableLoss, DestructiveOperation, ProtectionBlocker, WorktreeReport};

/// 保護ゲートが観測した結果。
///
/// fieldはすべて非公開とし、表示用のread-only accessorだけを公開する。
#[derive(Debug, Clone)]
pub struct ProtectionAssessment {
    operation: DestructiveOperation,
    sandbox: SandboxName,
    worktrees: Vec<WorktreeReport>,
    blockers: Vec<ProtectionBlocker>,
    confirmable_losses: Vec<ConfirmableLoss>,
}

impl ProtectionAssessment {
    /// 何も観測しなかった結果。未作成、またはcloneしたrepositoryをまだ持たないsandboxで使う。
    pub fn empty(operation: DestructiveOperation, sandbox: SandboxName) -> ProtectionAssessment {
        ProtectionAssessment::new(operation, sandbox, Vec::new(), Vec::new(), Vec::new())
    }

    pub(super) fn new(
        operation: DestructiveOperation,
        sandbox: SandboxName,
        worktrees: Vec<WorktreeReport>,
        blockers: Vec<ProtectionBlocker>,
        confirmable_losses: Vec<ConfirmableLoss>,
    ) -> ProtectionAssessment {
        ProtectionAssessment {
            operation,
            sandbox,
            worktrees,
            blockers,
            confirmable_losses,
        }
    }

    pub fn operation(&self) -> DestructiveOperation {
        self.operation
    }

    pub fn sandbox(&self) -> &SandboxName {
        &self.sandbox
    }

    pub fn worktrees(&self) -> &[WorktreeReport] {
        &self.worktrees
    }

    pub fn blockers(&self) -> &[ProtectionBlocker] {
        &self.blockers
    }

    /// 確認すれば削除してよい対象。`blockers()`が1件でもあれば、削除計画にも明示
    /// 確認にも進まないため、呼び出し側はこの一覧を見る前に`blockers()`が空である
    /// ことを確かめる。
    pub fn confirmable_losses(&self) -> &[ConfirmableLoss] {
        &self.confirmable_losses
    }
}

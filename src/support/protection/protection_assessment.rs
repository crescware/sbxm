use super::{ProtectionBlocker, WorktreeReport};

/// 保護ゲートが観測した結果。
///
/// fieldはすべて非公開とし、表示用のread-only accessorだけを公開する。
#[derive(Debug, Clone)]
pub struct ProtectionAssessment {
    worktrees: Vec<WorktreeReport>,
    blockers: Vec<ProtectionBlocker>,
}

impl ProtectionAssessment {
    pub(super) fn new(
        worktrees: Vec<WorktreeReport>,
        blockers: Vec<ProtectionBlocker>,
    ) -> ProtectionAssessment {
        ProtectionAssessment {
            worktrees,
            blockers,
        }
    }

    pub fn worktrees(&self) -> &[WorktreeReport] {
        &self.worktrees
    }

    pub fn blockers(&self) -> &[ProtectionBlocker] {
        &self.blockers
    }
}

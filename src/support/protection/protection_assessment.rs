use super::{ProtectionBlocker, WorktreeReport};

/// 保護ゲートが観測した結果。
///
/// fieldはすべて非公開とし、表示用のread-only accessorだけを公開する。
#[derive(Debug, Clone)]
pub struct ProtectionAssessment {
    project: String,
    worktrees: Vec<WorktreeReport>,
    blockers: Vec<ProtectionBlocker>,
}

impl ProtectionAssessment {
    pub(super) fn new(
        project: String,
        worktrees: Vec<WorktreeReport>,
        blockers: Vec<ProtectionBlocker>,
    ) -> ProtectionAssessment {
        ProtectionAssessment {
            project,
            worktrees,
            blockers,
        }
    }

    pub(super) fn project(&self) -> &str {
        &self.project
    }

    pub fn worktrees(&self) -> &[WorktreeReport] {
        &self.worktrees
    }

    pub fn blockers(&self) -> &[ProtectionBlocker] {
        &self.blockers
    }
}

use crate::project::SandboxName;

use super::{Blocker, ConfirmableLoss, DestructiveOperation, OriginObservation, WorktreeReport};

/// 保護ゲートが観測した結果。
///
/// fieldはすべて非公開とし、表示用のread-only accessorだけを公開する。
#[derive(Debug, Clone)]
pub struct Assessment {
    operation: DestructiveOperation,
    project: String,
    sandbox: SandboxName,
    worktrees: Vec<WorktreeReport>,
    blockers: Vec<Blocker>,
    confirmable_losses: Vec<ConfirmableLoss>,
    /// origin観測結果。何も観測しなかった`empty`では`None`。
    origin: Option<OriginObservation>,
}

impl Assessment {
    /// 何も観測しなかった結果。Sandboxがそもそも無い案件で使う。
    ///
    /// `gate::assess_absent`だけが呼ぶ。commandがここから`Assessment`を組み立てられると、
    /// 観測を1回も経ずに一致するsnapshotの組を作れてしまう。
    pub(super) fn empty(
        operation: DestructiveOperation,
        project: String,
        sandbox: SandboxName,
    ) -> Assessment {
        Assessment::new(
            operation,
            project,
            sandbox,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
        )
    }

    pub(super) fn new(
        operation: DestructiveOperation,
        project: String,
        sandbox: SandboxName,
        worktrees: Vec<WorktreeReport>,
        blockers: Vec<Blocker>,
        confirmable_losses: Vec<ConfirmableLoss>,
        origin: Option<OriginObservation>,
    ) -> Assessment {
        Assessment {
            operation,
            project,
            sandbox,
            worktrees,
            blockers,
            confirmable_losses,
            origin,
        }
    }

    pub fn operation(&self) -> DestructiveOperation {
        self.operation
    }

    pub(super) fn project(&self) -> &str {
        &self.project
    }

    pub fn sandbox(&self) -> &SandboxName {
        &self.sandbox
    }

    pub fn worktrees(&self) -> &[WorktreeReport] {
        &self.worktrees
    }

    pub fn blockers(&self) -> &[Blocker] {
        &self.blockers
    }

    /// 確認すれば削除してよい対象。`blockers()`が1件でもあれば、削除計画にも明示
    /// 確認にも進まないため、呼び出し側はこの一覧を見る前に`blockers()`が空である
    /// ことを確かめる。
    pub fn confirmable_losses(&self) -> &[ConfirmableLoss] {
        &self.confirmable_losses
    }

    /// origin観測結果。fingerprintが確認後のorigin側の変化を検出するために読む。
    ///
    /// 表示はworktreeごとの`Reachability`が担うため、この観測そのものは外へ出さない。
    pub(super) fn origin_observation(&self) -> Option<&OriginObservation> {
        self.origin.as_ref()
    }
}

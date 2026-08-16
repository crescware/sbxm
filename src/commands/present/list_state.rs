use crate::design::{Inline, VisualState};
use crate::support::inventory::{Observed, ProjectState, WorkspaceState};

use super::{observed, project_state};

/// `sbxm ls`が利用者へ見せる1案件の状態。
///
/// runtimeが持つSandboxの状態と、host上のworkspace directoryの実在は内部では別々に
/// 観測する。ただし一覧で利用者が知りたいのは、`open`をそのまま実行できるかである。
/// `open-blocked`はその2つの観測から得た利用者向けの結論であり、原因や対処commandを
/// 状態名へ埋め込まない。詳しい理由は`status`と診断が示す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListState {
    Missing,
    Incomplete,
    Inconsistent,
    Running,
    Stopped,
    OpenBlocked,
    NotObserved,
    NotCreated,
}

impl ListState {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            ListState::Missing => "missing",
            ListState::Incomplete => "incomplete",
            ListState::Inconsistent => "inconsistent",
            ListState::Running => "running",
            ListState::Stopped => "stopped",
            ListState::OpenBlocked => "open-blocked",
            ListState::NotObserved => "not-observed",
            ListState::NotCreated => "not-created",
        }
    }

    /// 凡例に使うFTL message ID。
    pub fn legend_id(self) -> &'static str {
        match self {
            ListState::Missing => "legend-missing",
            ListState::Incomplete => "legend-incomplete",
            ListState::Inconsistent => "legend-inconsistent",
            ListState::Running => "legend-sandbox-running",
            ListState::Stopped => "legend-sandbox-stopped",
            ListState::OpenBlocked => "legend-open-blocked",
            ListState::NotObserved => "legend-not-observed",
            ListState::NotCreated => "legend-not-created",
        }
    }

    /// 内部の観測結果を、一覧で使う利用者向けの状態へ写す。
    pub fn from_observation(observed: &Observed, workspace: WorkspaceState) -> Self {
        match observed {
            Observed::Missing => ListState::Missing,
            Observed::Incomplete => ListState::Incomplete,
            Observed::Inconsistent => ListState::Inconsistent,
            Observed::Registered(ProjectState::Running) => {
                // runningのSandboxへopenする経路は、停止中のSandboxのようにhost側の
                // workspaceを起動前提として検査しない。workspaceの異常はstatusで示す。
                ListState::Running
            }
            Observed::Registered(ProjectState::Stopped) => match workspace {
                WorkspaceState::Ready => ListState::Stopped,
                WorkspaceState::Missing => ListState::OpenBlocked,
                // openがそのまま進めるかを確定できないため、missingとは断定しない。
                WorkspaceState::NotObserved | WorkspaceState::NotApplicable => {
                    ListState::NotObserved
                }
            },
            Observed::Registered(ProjectState::NotCreated) => ListState::NotCreated,
        }
    }

    /// `ls`の利用者向け状態を、一覧の文脈に合う色で示す。
    pub fn render(self) -> Inline {
        match self {
            ListState::Missing => observed(&Observed::Missing),
            ListState::Incomplete => observed(&Observed::Incomplete),
            ListState::Inconsistent => observed(&Observed::Inconsistent),
            ListState::Running => project_state(ProjectState::Running),
            ListState::Stopped => project_state(ProjectState::Stopped),
            ListState::NotCreated => project_state(ProjectState::NotCreated),
            ListState::OpenBlocked | ListState::NotObserved => {
                Inline::state(self.as_str(), VisualState::Attention)
            }
        }
    }
}

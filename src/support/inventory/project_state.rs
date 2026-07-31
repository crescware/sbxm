/// 利用者へ見せるSandboxの状態。
///
/// `registered`は内部の管理状態名であり、対応Sandboxがまだない同じ状態を
/// 利用者向けには`not-created`と表示する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectState {
    NotCreated,
    Running,
    Stopped,
}

impl ProjectState {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectState::NotCreated => "not-created",
            ProjectState::Running => "running",
            ProjectState::Stopped => "stopped",
        }
    }

    /// 凡例に使うFTL message ID。host serviceではなくSandboxの状態を説明する。
    pub fn legend_id(self) -> &'static str {
        match self {
            ProjectState::NotCreated => "legend-not-created",
            ProjectState::Running => "legend-sandbox-running",
            ProjectState::Stopped => "legend-sandbox-stopped",
        }
    }
}

impl std::fmt::Display for ProjectState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Sandboxのruntime状態。未知の値を既知の値へ丸めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxState {
    Running,
    Stopped,
}

impl SandboxState {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            SandboxState::Running => "running",
            SandboxState::Stopped => "stopped",
        }
    }
}

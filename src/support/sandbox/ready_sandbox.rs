use std::path::PathBuf;

use crate::compatibility::SandboxState;

/// 使用できる状態のSandbox。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadySandbox {
    pub name: String,
    pub workspace: PathBuf,
    pub state: SandboxState,
    /// この実行で作成したか。
    pub created: bool,
}

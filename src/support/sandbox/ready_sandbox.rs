use std::path::PathBuf;

use crate::boundary::host::protocol::SandboxState;

/// 使用できる状態のSandbox。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadySandbox {
    pub name: String,
    pub workspace: PathBuf,
    pub state: SandboxState,
    /// この実行で作成したか。
    pub created: bool,
    /// 既にあるSandboxのworkspace directoryが消えていて、この実行で作り直したか。
    ///
    /// 作り直したのは中立なmount点であり、Sandboxの中にあるrepositoryではない。
    pub workspace_restored: bool,
}

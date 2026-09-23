use super::FileState;

/// 宣言file 1件の、hostとSandboxそれぞれの状態。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    /// Sandboxの`agent` homeからの配置先。
    pub destination: String,
    pub host: FileState,
    pub sandbox: FileState,
}

#[derive(Clone)]
pub struct SandboxRow {
    pub name: String,
    pub workspace: String,
    /// runtimeが答えるstate。停止中のSandboxは中を読めない。
    pub running: bool,
}

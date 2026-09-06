#[derive(Clone)]
pub struct SandboxRow {
    pub name: String,
    pub workspace: String,
    /// 作成時にcustom secretが登録済みだったか。実物と同じく、あとから登録しても
    /// 既に存在するSandboxへはplaceholderが届かない。
    pub placeholder: bool,
    /// runtimeが答えるstate。停止中のSandboxは中を読めない。
    pub running: bool,
}

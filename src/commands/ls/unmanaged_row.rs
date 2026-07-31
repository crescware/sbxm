/// 管理外Sandboxの1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmanagedRow {
    pub sandbox: String,
    /// runtimeが示したままのstate。sbxmのenumへ写像しない。
    pub state: String,
    pub workspace: String,
}

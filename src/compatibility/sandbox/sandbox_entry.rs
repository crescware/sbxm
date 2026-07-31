use super::SandboxState;

/// `sbx ls`が示すSandbox 1件。
///
/// 対応versionの`sbx ls --json`は、元にしたTemplateも接続中のsession数も示さない。
/// 示されない値をOptionで持つと、その`None`をどう読むかを利用側がそれぞれ決めることに
/// なる。読めないものは持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxEntry {
    pub name: String,
    pub state: SandboxState,
    /// runtimeが示したままのstate。管理外Sandboxの表示に使う。
    pub raw_state: String,
    /// Sandboxへ渡したworkspace。示されない場合は`None`。
    pub workspace: Option<String>,
}

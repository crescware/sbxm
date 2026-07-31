/// 既存のdestinationと内容が異なる場合の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    /// `add`。構築の途中で利用者のfileを上書きしない。
    Refuse,
    /// `sync-files`。現在のglobal configを明示的な再配置要求として扱う。
    Overwrite,
}

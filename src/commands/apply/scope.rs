/// 何を適用するか。
///
/// 省略した対象は変更しない。宣言fileの配置は既存のfileを上書きするため、暗黙には
/// 走らせない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scope {
    /// global configが宣言するfileを再配置する。
    pub files: bool,
    /// managed worktreeの目標本数。現在より多い値だけを受け付ける。
    pub worktrees: Option<u32>,
}

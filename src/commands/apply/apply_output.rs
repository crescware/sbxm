use crate::support::files::PlacedFile;

/// `apply`の結果。
#[derive(Debug, Clone)]
pub struct ApplyOutput {
    pub project: String,
    pub sandbox: String,
    pub files: Vec<PlacedFile>,
    /// worktreeを適用した場合の、適用後の本数。
    pub worktrees: Option<u32>,
}

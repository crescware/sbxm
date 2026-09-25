use std::path::PathBuf;

use super::SentChange;

/// `send`の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendOutput {
    pub project: String,
    /// 送り元のhostのrepository。
    pub repository: PathBuf,
    /// Sandboxのorigin側で変わったref。何も変わらなければ空。
    pub changes: Vec<SentChange>,
}

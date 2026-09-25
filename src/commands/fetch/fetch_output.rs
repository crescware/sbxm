use std::path::PathBuf;

use crate::support::bundle::RefChange;

/// `fetch`の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchOutput {
    pub project: String,
    /// 保存先のhostのrepository。
    pub repository: PathBuf,
    /// 保存先の名前空間。`refs/sbx/<namespace>/`。
    pub namespace: String,
    /// 書き換えたref。Sandboxが保存するrefを持たなかった場合は`None`。
    pub changes: Option<Vec<RefChange>>,
}

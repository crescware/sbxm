use std::path::PathBuf;

/// Sandboxから受け取った宣言fileの内容。hostの隔離領域に置かれている。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedCopy {
    pub path: PathBuf,
    pub sha256: String,
}

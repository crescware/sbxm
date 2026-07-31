use std::path::PathBuf;

use super::Placement;

/// 1件の宣言に対する結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedFile {
    pub source: PathBuf,
    /// `agent` homeからの相対path。
    pub destination: String,
    pub placement: Placement,
}

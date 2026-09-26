/// hostのrepositoryへ保存済みの、Sandboxのref 1件の先端。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedTip {
    /// hostのrepositoryでの完全なref名。`refs/sbx/<sandbox>/...`。
    pub reference: String,
    pub commit: String,
}

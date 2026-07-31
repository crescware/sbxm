use super::{Error, ErrorId, Result, fail};
use crate::msg;

/// versionを持つ文書の、versionの読み方。
///
/// 文書ごとに1度宣言し、parseの先頭で`require`を呼ぶ。
pub struct DocumentVersion {
    /// 読める唯一のversion。
    pub supported: u32,
    /// 未知versionのerror ID。
    pub unknown: ErrorId,
    /// `未知versionのFTL` message ID。
    pub unknown_message: &'static str,
}

impl DocumentVersion {
    /// versionを最初に確定させ、未知versionを他の項目より前に診断する。
    ///
    /// `path`は表示用に整えたもの、`missing`はversion欄自体が無い場合の診断とする。
    pub fn require(
        &self,
        found: Option<i64>,
        path: &str,
        missing: impl FnOnce() -> Error,
    ) -> Result<()> {
        match found {
            Some(version) if version == i64::from(self.supported) => Ok(()),
            Some(version) => fail(
                self.unknown,
                msg!(
                    self.unknown_message,
                    path = path,
                    version = version,
                    supported = self.supported
                ),
            ),
            None => Err(missing()),
        }
    }
}

use super::*;

impl GlyphSet {
    /// 定義した全glyph。testが一覧を取りこぼさないよう、fieldの宣言と対にして置く。
    pub(crate) fn all(self) -> [&'static str; 7] {
        [
            self.progress,
            self.success,
            self.warning,
            self.error,
            self.current,
            self.arrow_up,
            self.arrow_down,
        ]
    }
}

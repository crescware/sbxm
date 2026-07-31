/// markerに使う文字。
///
/// 一つの前景色で描画されるtext symbolだけを持つ。二色以上のpictographとして描画され
/// 得る文字、variation selector、ZWJ sequence、regional indicator、keycap sequenceは
/// 追加しない。現在のterminalで単色に見えることを許可理由にしない。
#[derive(Debug, Clone, Copy)]
pub struct GlyphSet {
    pub progress: &'static str,
    pub success: &'static str,
    pub warning: &'static str,
    pub error: &'static str,
    pub current: &'static str,
    /// promptの操作説明が指す上下キー。key名は翻訳しない。
    pub arrow_up: &'static str,
    pub arrow_down: &'static str,
}

impl GlyphSet {
    /// 定義した全glyph。testが一覧を取りこぼさないよう、宣言と同じ場所から作る。
    #[cfg(test)]
    pub(super) fn all(self) -> [&'static str; 7] {
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

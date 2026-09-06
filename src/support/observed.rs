/// artifact 1件の観測結果。
///
/// 「present/matchesの2つのbool」のような場当たり的な組み合わせを避け、欠落・一致・
/// 食い違い・観測不能の4状態を型で分ける。`Mismatch`と`Unobservable`はevidenceを
/// 持ち、診断へそのまま転記できる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    /// 存在しない。まだ作られていない、または消えている。
    Missing,
    /// 期待する値と一致することを確認できた。
    Matching,
    /// 存在するが、期待する値と異なることを確認できた。
    Mismatch { evidence: String },
    /// 存在するかどうか、または値が一致するかどうかを確認できなかった。
    Unobservable { evidence: String },
}

impl Observed {
    pub fn is_matching(&self) -> bool {
        matches!(self, Observed::Matching)
    }

    /// 無いことを確認できた。観測できなかった場合と混ぜない。
    pub fn is_missing(&self) -> bool {
        matches!(self, Observed::Missing)
    }

    /// 翻訳しない安定した表記。
    pub fn as_str(&self) -> &'static str {
        match self {
            Observed::Missing => "missing",
            Observed::Matching => "matching",
            Observed::Mismatch { .. } => "mismatch",
            Observed::Unobservable { .. } => "not-observed",
        }
    }
}

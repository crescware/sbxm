/// 1件の配置結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// 配置した。
    Placed,
    /// 既に同じ内容だったため何もしなかった。
    Unchanged,
}

impl Placement {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            Placement::Placed => "placed",
            Placement::Unchanged => "unchanged",
        }
    }
}

/// 状態値が文脈上どちらへ倒れているか。
///
/// 同じ文字列でも文脈で意味が変わる。`stopped`は停止commandの完了結果ならpositive、
/// 稼働要件のstatusならattentionである。値から色を推測せず、出力modelが明示する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualState {
    Positive,
    Attention,
    Negative,
    Neutral,
}

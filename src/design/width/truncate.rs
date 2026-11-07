use super::shorten;

/// 幅に収まらない文字列を、末尾を省略記号へ置き換えて縮める。
///
/// 横折り返しで次の候補に見えてしまうのを避けるため、promptのlabelへ使う。
pub fn truncate(text: &str, width: usize) -> String {
    let (kept, ellipsis) = shorten(text, width);
    format!("{kept}{ellipsis}")
}

use super::{display_width, is_wide};

/// 幅に収まらない文字列を、末尾を省略記号へ置き換えて縮める。
///
/// 横折り返しで次の候補に見えてしまうのを避けるため、promptのlabelへ使う。
pub fn truncate(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_string();
    }
    // 省略記号そのものが入らない幅では、切り詰めだけを行う。
    let ellipsis = "...";
    let budget = width.saturating_sub(display_width(ellipsis));
    if budget == 0 {
        return take_width(text, width);
    }
    format!("{}{ellipsis}", take_width(text, budget))
}

/// 表示幅が上限を超えない範囲の先頭部分。
fn take_width(text: &str, width: usize) -> String {
    let mut taken = String::new();
    let mut used = 0;
    for character in text.chars() {
        let next = if is_wide(character) { 2 } else { 1 };
        if used + next > width {
            break;
        }
        taken.push(character);
        used += next;
    }
    taken
}

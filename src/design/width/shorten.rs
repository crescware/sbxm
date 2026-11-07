use super::{display_width, is_wide};

/// 幅に収まらない文字列を、残す先頭部分と省略記号へ分ける。
///
/// 残す部分は元の文字列の先頭をそのまま指すため、呼び出し側は元の文字列で求めた位置を
/// そのまま使える。収まる文字列は全体と空の省略記号を返す。省略記号そのものが入らない
/// 幅では、切り詰めだけを行う。
pub fn shorten(text: &str, width: usize) -> (&str, &'static str) {
    if display_width(text) <= width {
        return (text, "");
    }
    let ellipsis = "...";
    let budget = width.saturating_sub(display_width(ellipsis));
    if budget == 0 {
        return (take_width(text, width), "");
    }
    (take_width(text, budget), ellipsis)
}

/// 表示幅が上限を超えない範囲の先頭部分。
fn take_width(text: &str, width: usize) -> &str {
    let mut used = 0;
    let mut end = 0;
    for (offset, character) in text.char_indices() {
        let next = if is_wide(character) { 2 } else { 1 };
        if used + next > width {
            break;
        }
        used += next;
        end = offset + character.len_utf8();
    }
    &text[..end]
}

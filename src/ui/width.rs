//! 表示幅と、列をそろえる整形。
//!
//! 幅はANSIを含まない元文字列から数える。装飾を付けたあとの文字列で数えると、色の
//! on/offで列の開始位置がずれる。整形を先に確定させ、装飾はそのあとで載せる。

/// 全角文字を2桁として数える表示幅。
///
/// 日本語modeでも列を揃えるために使う。日本語modeのstdoutは機械可読な出力契約としない。
pub(super) fn display_width(text: &str) -> usize {
    text.chars()
        .map(|character| if is_wide(character) { 2 } else { 1 })
        .sum()
}

fn is_wide(character: char) -> bool {
    matches!(character as u32,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x20000..=0x3FFFD)
}

/// 指定幅へ届くまでの余白。装飾を載せた文字列へ後から連結する。
pub(super) fn padding(text: &str, width: usize) -> String {
    " ".repeat(width.saturating_sub(display_width(text)))
}

/// 幅に収まらない文字列を、末尾を省略記号へ置き換えて縮める。
///
/// 横折り返しで次の候補に見えてしまうのを避けるため、promptのlabelへ使う。
pub(super) fn truncate(text: &str, width: usize) -> String {
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

#[cfg(test)]
#[path = "width_test.rs"]
mod width_test;

//! 表示幅と、列をそろえる整形。

/// 全角文字を2桁として数える表示幅。
///
/// 日本語modeでも列を揃えるために使う。日本語modeのstdoutは機械可読な出力契約としない。
pub fn display_width(text: &str) -> usize {
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

/// 1行分を、列幅にそろえて描画する。末尾の余白は残さない。
pub(super) fn render_row(values: &[String], widths: &[usize]) -> String {
    let mut line = String::new();
    for (index, value) in values.iter().enumerate() {
        if index + 1 == values.len() {
            line.push_str(value);
        } else {
            line.push_str(&pad_to(value, widths[index]));
        }
    }
    line.push('\n');
    line
}

pub(super) fn pad_to(text: &str, width: usize) -> String {
    let mut out = text.to_string();
    for _ in display_width(text)..width {
        out.push(' ');
    }
    out
}

#[cfg(test)]
#[path = "width_test.rs"]
mod width_test;

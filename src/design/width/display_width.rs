use super::is_wide;

/// 全角文字を2桁として数える表示幅。
///
/// 日本語modeでも列を揃えるために使う。日本語modeのstdoutは機械可読な出力契約としない。
pub fn display_width(text: &str) -> usize {
    text.chars()
        .map(|character| if is_wide(character) { 2 } else { 1 })
        .sum()
}

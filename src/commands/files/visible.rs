/// 端末へ出す前に、制御文字と文字の向きを変える文字を見える表記へ置き換える。
///
/// Sandboxから受け取った内容は信用しない。escape sequenceで行を消したり、向きを変える
/// 文字で並びを偽ったりすれば、利用者は見たものと違う内容を採用しうる。改行とtabだけは
/// そのまま残す。
pub fn visible(text: &str) -> String {
    use std::fmt::Write as _;

    let mut shown = String::with_capacity(text.len());
    for character in text.chars() {
        let bidirectional = matches!(
            character,
            '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        );
        if (character.is_control() && character != '\n' && character != '\t') || bidirectional {
            // Stringへの書き込みは失敗しない。
            let _ = write!(shown, "\\u{{{:x}}}", u32::from(character));
        } else {
            shown.push(character);
        }
    }
    shown
}

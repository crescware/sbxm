/// Sandboxから届いた文字列を、hostの端末へそのまま出せる形にする。
///
/// Sandboxの中のprogramは、hostの端末を操作するescape sequenceや、表示を書き戻す`\r`を
/// 送れる。改行とtabを除く制御文字を、`\u{1b}`のように見える形へ置き換える。
pub fn neutralized(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control() && character != '\n' && character != '\t' {
            shown.extend(character.escape_default());
        } else {
            shown.push(character);
        }
    }
    shown
}

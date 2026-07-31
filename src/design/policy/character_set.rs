/// markerと罫線に使える文字の範囲。
///
/// localeから推測しない。Unicodeを既定とし、`TERM=dumb`のように端末側が表示を
/// 保証できない場合だけASCIIへ落とす。罫線を意味の必須要素にしないため、どちらでも
/// 情報は同じである。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterSet {
    Unicode,
    Ascii,
}

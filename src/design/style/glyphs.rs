use crate::design::policy::CharacterSet;

use super::{ASCII, GlyphSet};

const UNICODE: GlyphSet = GlyphSet {
    progress: "\u{2192}",
    success: "\u{2713}",
    warning: "!",
    error: "\u{00d7}",
    current: "\u{203a}",
    arrow_up: "\u{2191}",
    arrow_down: "\u{2193}",
};

/// 文字集合に対応するglyph。どちらでも意味は変わらない。
pub fn glyphs(characters: CharacterSet) -> GlyphSet {
    match characters {
        CharacterSet::Unicode => UNICODE,
        CharacterSet::Ascii => ASCII,
    }
}

//! 意味から端末styleへの唯一の写像。
//!
//! callerは`red`や`cyan`を指定しない。何を意味するかだけを[`Role`]と[`VisualState`]で
//! 示し、具体色はここだけが決める。同じ意味が全commandと全localeで同じ見え方になるのは
//! この一方向の写像があるためであり、command固有の色を作れないのもそのためである。
//!
//! `具体色はANSI標準16色のnamed` colorだけを表す。固定RGBや256色indexを持たないのは
//! 機能不足ではなく、利用者のterminal themeとcontrast設定を尊重するための制約である。

mod ascii;
mod color;
mod glyph_set;
mod glyphs;
mod role;
mod role_style;
mod state_style;
mod style_spec;
mod visual_state;

use ascii::ASCII;
pub(super) use color::Color;
pub(super) use glyph_set::GlyphSet;
pub(super) use glyphs::glyphs;
pub use role::Role;
pub(super) use role_style::role_style;
pub(super) use state_style::state_style;
pub(super) use style_spec::StyleSpec;
pub use visual_state::VisualState;

#[cfg(test)]
#[path = "style_test.rs"]
mod style_test;

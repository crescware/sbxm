//! helpの組み立て。
//!
//! CLI parser libraryの英語固定表記へ委ねず、選択したlocaleでheadingとhelp textを
//! 組む。templateはcommandが持つ引数の種類ごとに決まる。

mod builder;
mod format;
mod text;

pub use builder::Builder;
use format::format;
use text::text;

//! locale依存のhelp textを、parser libraryから独立して組み立てる。

mod builder;
mod format;
mod text;

pub use builder::Builder;
use format::format;
use text::text;

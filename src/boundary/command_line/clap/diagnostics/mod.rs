//! CLI parser libraryのerrorを、翻訳した説明と安定したerror IDへ写像する。

mod context_string;
mod interpret;
mod map;

use context_string::context_string;
pub(crate) use interpret::interpret;

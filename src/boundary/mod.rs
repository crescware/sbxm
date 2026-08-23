//! applicationと外界の境界。
//!
//! clapやterminal、host processの具体的な型はこのmoduleの内側へ閉じ込め、application
//! commandへは境界の値だけを渡す。

pub mod command_line;
pub mod host;
pub(crate) mod terminal;

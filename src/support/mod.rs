//! Commandが共有するworkflow部品と、利用者向け出力の共通部分。
//!
//! ここへ置くのは複数のcommandから呼ばれるものだけとし、1 commandからしか呼ばれない
//! ものは`crate::commands`のcommand directoryが持つ。

pub mod daemon;
pub mod display;
pub mod files;
pub mod generation;
pub mod identity;
pub mod image;
pub mod inventory;
pub mod protection;
pub mod reporter;
pub mod repository;
pub mod sandbox;
pub mod secret;
pub mod select;
pub mod status;
pub mod template;
pub mod tools;
pub mod width;
pub mod worktree;

pub use reporter::Reporter;
pub use status::{Row, StatusValue};

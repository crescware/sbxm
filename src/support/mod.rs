//! Commandが共有するworkflow部品。
//!
//! ここへ置くのは複数のcommandから呼ばれるものだけとし、1 commandからしか呼ばれない
//! ものは`crate::commands`のcommand directoryが持つ。
//!
//! 利用者向けの描画は持たない。表示の語彙、色、blockの間隔、promptは`crate::design`が
//! application全体のinterfaceとして持つ。

pub mod daemon;
pub mod disk;
pub mod docker;
pub mod files;
pub mod generation;
pub mod identity;
pub mod image;
pub mod inventory;
pub mod protection;
pub mod repository;
pub mod sandbox;
pub mod secret;
pub mod select;
pub mod status;
pub mod template;
pub mod tools;
pub mod worktree;

pub use status::{Row, StatusValue};

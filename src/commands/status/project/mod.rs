//! `sbxm status <owner>/<repository>`。
//!
//! 1案件の構築状態、作業可能性、credential隔離をread-onlyで診断する。repair、起動、
//! 停止、file更新を行わない。作成元やsbxm独自のmarkerは検査せず、現在の状態だけを見る。

mod artifacts;
mod diagnose;
#[cfg(test)]
mod fake;
mod inside;
mod item;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod project_status;
mod repository;
mod value;
mod worktree_row;

pub use diagnose::diagnose;
pub use item::Item;
pub use project_status::ProjectStatus;
pub use value::Value;
pub use worktree_row::WorktreeRow;

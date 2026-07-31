//! 共通のデータ保護検査。
//!
//! runningなSandboxを削除する通常modeの`rebuild`と`destroy`は、同じ列挙と判定規則で
//! 保存されていない作業がないことを確かめる。判定できない場合は削除しない。

mod answered;
mod inspect;
mod kind;
mod mode;
mod remote;
mod report;
mod unmanaged;
mod worktree_report;

use answered::answered;
pub use inspect::inspect;
pub use kind::Kind;
pub use mode::Mode;
pub use remote::Remote;
pub use report::Report;
pub use unmanaged::Unmanaged;
pub use worktree_report::WorktreeReport;

#[cfg(test)]
#[path = "protection_test.rs"]
mod protection_test;

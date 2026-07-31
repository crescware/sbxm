//! `sbxm status --global`。
//!
//! hostとglobal環境をread-onlyで診断する。login、setup、file更新、daemon起動・停止を
//! 行わない。問題がある場合は、利用者が直接実行する外部commandを表示する。
//!
//! 検査対象は、sbxm自身がhost上で直接使用する設定、platform、command、serviceに限定する。

mod diagnose;
mod external;
mod global_status;
mod host_commands;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod platform;
mod push;
mod sandboxes;
mod service;
mod settings;

pub use diagnose::diagnose;
pub use global_status::GlobalStatus;
use push::push;

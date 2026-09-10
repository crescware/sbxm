//! `sbxm open`。
//!
//! 登録済み案件を接続可能な状態まで準備し、SSHでterminalを引き渡す。

mod clamped_index;
mod connect;
mod prepare;
mod prepared;

pub use clamped_index::ClampedIndex;
pub use connect::connect;
pub use prepare::prepare;
pub use prepared::Prepared;

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

#[cfg(test)]
#[path = "provisioning_test.rs"]
mod provisioning_test;

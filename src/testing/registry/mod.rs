//! registryのtestが共有するfixture。
//!
//! sbxmが書き出すのと同じ形でentryを組み立てる。

mod document;
mod entry;

pub use document::document;
pub use entry::Entry;

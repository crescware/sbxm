//! 保護の検査を通るhost。

mod clean_host;
mod commit_only_in_the_sandbox;

pub use clean_host::clean_host;
pub use commit_only_in_the_sandbox::commit_only_in_the_sandbox;

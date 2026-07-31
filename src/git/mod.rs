//! Git参照の検証と表記。
//!
//! 利用者が指定するのはremote branch名だけであり、sbxmが組み立てるのは
//! `refs/remotes/origin/<branch>`という完全なremote-tracking refである。

mod canonical_id_of_remote;
mod github_host;
mod https_remote_url;
mod origin_ref;
mod require_github;
mod validate_branch_name;

pub use canonical_id_of_remote::canonical_id_of_remote;
use github_host::GITHUB_HOST;
pub use https_remote_url::https_remote_url;
pub use origin_ref::origin_ref;
use require_github::require_github;
pub use validate_branch_name::validate_branch_name;

#[cfg(test)]
#[path = "git_test.rs"]
mod git_test;

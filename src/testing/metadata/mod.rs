//! Project metadataのtestが共有するfixture。

mod attached;
mod canonical;
mod git_identity;
mod other_digest;

pub use attached::attached;
pub use canonical::canonical;
pub use git_identity::git_identity;
pub use other_digest::OTHER_DIGEST;

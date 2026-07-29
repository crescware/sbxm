//! testが共有するfixture。
//!
//! moduleを跨いで使うものだけを置く。1つのtest fileの中だけで完結するfakeは、その
//! fileに残す。

pub mod add_request;
pub mod command;
pub mod host;
pub mod project;
pub mod prompt;
pub mod protection;
pub mod value;

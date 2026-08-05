//! testが共有するfixture。
//!
//! moduleを跨いで使うものだけを置く。1つのtest fileの中だけで完結するfakeは、その
//! fileに残す。

pub mod add_request;
pub mod archive;
pub mod cli;
pub mod command;
pub mod fs;
pub mod global_status;
pub mod host;
pub mod image;
pub mod metadata;
pub mod outcome;
mod plain;
pub mod poll;
pub mod project;
pub mod prompt;
pub mod protection;
pub mod recorded_output;
pub mod registry;
pub mod render;
pub mod repository;
pub mod sandbox;
pub mod value;

pub use plain::plain;

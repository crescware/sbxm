//! Dockerfileの世代。
//!
//! 世代はDockerfileの内容hashで表す。`rebuild`は切替の途中でintentを残すため、工程を
//! 進めるcommandは進める前にintentの不在を確かめる。

mod current_dockerfile_hash;
mod require_no_rebuild;

pub use current_dockerfile_hash::current_dockerfile_hash;
pub use require_no_rebuild::require_no_rebuild;

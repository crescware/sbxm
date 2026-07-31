//! imageのtestが使うdocker fake。

mod canonical;
mod declared_labels;
mod fake_docker;
mod inspect_output;
mod matching_inspect;
mod sandbox;

pub use canonical::canonical;
pub use declared_labels::declared_labels;
pub use fake_docker::FakeDocker;
pub use inspect_output::inspect_output;
pub use matching_inspect::matching_inspect;
pub use sandbox::sandbox;

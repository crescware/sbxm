//! 外部commandの偽の応答。

mod inner_args;
mod outcome;
mod outcome_with_stderr;

pub use inner_args::inner_args;
pub use outcome::outcome;
pub use outcome_with_stderr::outcome_with_stderr;

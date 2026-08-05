//! 外部commandの偽の応答。

mod inner_args;
mod outcome;
mod outcome_with_stderr;
mod read_step;
mod scripted_pipe;

pub use inner_args::inner_args;
pub use outcome::outcome;
pub use outcome_with_stderr::outcome_with_stderr;
pub use read_step::ReadStep;
pub use scripted_pipe::ScriptedPipe;

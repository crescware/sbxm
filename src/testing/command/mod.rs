//! 外部commandの偽の応答と、台本どおりに答えるOS。

mod end;
mod event;
mod inner_args;
mod outcome;
mod outcome_with_stderr;
mod read_step;
mod scripted_controller;
mod scripted_os;
mod scripted_pipe;
mod scripted_writer;
mod step;

pub use end::End;
pub use event::Event;
pub use inner_args::inner_args;
pub use outcome::outcome;
pub use outcome_with_stderr::outcome_with_stderr;
pub use read_step::ReadStep;
pub use scripted_controller::ScriptedController;
pub use scripted_os::ScriptedOs;
pub use scripted_pipe::ScriptedPipe;
pub use scripted_writer::ScriptedWriter;
pub use step::Step;

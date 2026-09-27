use std::io;
use std::process::{ChildStderr, ChildStdin, ChildStdout};
use std::time::Duration;

use rustix::event::{Nsecs, PollFd, PollFlags, Timespec};
use rustix::fs::OFlags;

use crate::boundary::host::Pipes;

use super::RealHost;

/// 待たずに読み書きする設定は、file status flagsを読み、`NONBLOCK`を足して書き戻す。
impl Pipes for RealHost {
    type Watch<'a> = PollFd<'a>;

    fn stdin_nonblocking(&self, pipe: &ChildStdin) -> io::Result<()> {
        Ok(rustix::fs::fcntl_setfl(
            pipe,
            rustix::fs::fcntl_getfl(pipe)? | OFlags::NONBLOCK,
        )?)
    }

    fn stdout_nonblocking(&self, pipe: &ChildStdout) -> io::Result<()> {
        Ok(rustix::fs::fcntl_setfl(
            pipe,
            rustix::fs::fcntl_getfl(pipe)? | OFlags::NONBLOCK,
        )?)
    }

    fn stderr_nonblocking(&self, pipe: &ChildStderr) -> io::Result<()> {
        Ok(rustix::fs::fcntl_setfl(
            pipe,
            rustix::fs::fcntl_getfl(pipe)? | OFlags::NONBLOCK,
        )?)
    }

    fn watch_stdin<'a>(&self, pipe: &'a ChildStdin) -> PollFd<'a> {
        PollFd::new(pipe, PollFlags::OUT | PollFlags::HUP | PollFlags::ERR)
    }

    fn watch_stdout<'a>(&self, pipe: &'a ChildStdout) -> PollFd<'a> {
        PollFd::new(
            pipe,
            PollFlags::IN | PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL,
        )
    }

    fn watch_stderr<'a>(&self, pipe: &'a ChildStderr) -> PollFd<'a> {
        PollFd::new(
            pipe,
            PollFlags::IN | PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL,
        )
    }

    fn wait_for_io(&self, watched: &mut [PollFd<'_>], within: Duration) -> io::Result<usize> {
        Ok(rustix::event::poll(
            watched,
            Some(&Timespec {
                tv_sec: u64::cast_signed(within.as_secs()),
                tv_nsec: Nsecs::from(within.subsec_nanos()),
            }),
        )?)
    }
}

#[cfg(test)]
#[path = "system_pipe_test.rs"]
mod system_pipe_test;

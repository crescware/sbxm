use std::io::Result as IoResult;
use std::os::fd::AsFd;

use super::WAIT_POLL_INTERVAL;

/// 2本の読み取り端のどちらが読めるかを、短い待ちで確かめる。
pub(super) fn poll_pipes<O: AsFd, E: AsFd>(
    stdout: Option<&O>,
    stderr: Option<&E>,
) -> IoResult<(bool, bool)> {
    let mut poll_fds = Vec::new();
    let mut stdout_index = None;
    let mut stderr_index = None;
    let events = rustix::event::PollFlags::IN
        | rustix::event::PollFlags::HUP
        | rustix::event::PollFlags::ERR
        | rustix::event::PollFlags::NVAL;
    if let Some(stdout) = stdout {
        stdout_index = Some(poll_fds.len());
        poll_fds.push(rustix::event::PollFd::new(stdout, events));
    }
    if let Some(stderr) = stderr {
        stderr_index = Some(poll_fds.len());
        poll_fds.push(rustix::event::PollFd::new(stderr, events));
    }

    match rustix::event::poll(
        &mut poll_fds,
        Some(&rustix::event::Timespec {
            tv_sec: 0,
            tv_nsec: WAIT_POLL_INTERVAL.subsec_nanos().into(),
        }),
    ) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
            return Ok((false, false));
        }
        Err(error) => return Err(error.into()),
    }

    let ready = |index: Option<usize>| {
        index.is_some_and(|index| poll_fds[index].revents().intersects(events))
    };
    Ok((ready(stdout_index), ready(stderr_index)))
}

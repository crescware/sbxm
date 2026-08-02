use std::io::{Read, Result as IoResult};
use std::os::fd::AsFd;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{CommandSpec, SignalGuard, WAIT_POLL_INTERVAL, spawn_failure, terminate_child};

/// Capture commandのprocessと2つのpipeを、親threadだけで回収する。
///
/// pipeを読むthreadを残すと、子孫がpipeの書き込み端を引き継いだときにjoinが終わらない。
/// pipeをnonblockingにして子processの待機と同じloopで読むことで、子processが終わった時点で
/// 読み取り端を閉じられる。元のprocess groupから逃げた子孫の出力は、その時点で捨てる。
pub(super) fn run_capture(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    signal: &SignalGuard,
) -> Result<(ExitStatus, Vec<u8>, Vec<u8>)> {
    let Some(stdout) = child.stdout.take() else {
        terminate_child(child, spec);
        return Err(unreadable(spec, "stdout pipe was not created"));
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_child(child, spec);
        return Err(unreadable(spec, "stderr pipe was not created"));
    };
    let nonblocking = set_nonblocking(&stdout).and_then(|()| set_nonblocking(&stderr));
    if let Err(error) = nonblocking {
        terminate_child(child, spec);
        return Err(unreadable(spec, &error.to_string()));
    }
    collect_until_exit(child, spec, limit, signal, stdout, stderr)
}

/// 直接の子が終わるまで、2本のstreamをnonblockingで読み続ける。
///
/// このloopが相手に求めるのは、pollできてreadできることだけである。読めなくなった相手を
/// どうするか——まだ動いている子は終わらせてから報告し、既に終わった子の残りは諦める——も、
/// 相手がchildのpipeであるかどうかでは変わらない。読む相手を型で受け取り、pipeの生死を
/// OSに委ねずに決められるようにする。
fn collect_until_exit<O: Read + AsFd, E: Read + AsFd>(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    signal: &SignalGuard,
    stdout: O,
    stderr: E,
) -> Result<(ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut stdout = Some(stdout);
    let mut stderr = Some(stderr);

    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let deadline = limit.map(|duration| Instant::now() + duration);

    loop {
        if signal.interrupted() {
            terminate_child(child, spec);
            return Err(Error::Canceled);
        }

        let (stdout_ready, stderr_ready) =
            poll_pipes(stdout.as_ref(), stderr.as_ref()).map_err(|error| {
                terminate_child(child, spec);
                unreadable(spec, &error.to_string())
            })?;

        if stdout_ready {
            drain_pipe(&mut stdout, &mut stdout_bytes).map_err(|error| {
                terminate_child(child, spec);
                unreadable(spec, &error.to_string())
            })?;
        }
        if stderr_ready {
            drain_pipe(&mut stderr, &mut stderr_bytes).map_err(|error| {
                terminate_child(child, spec);
                unreadable(spec, &error.to_string())
            })?;
        }

        // 読んでいる間に届いたCtrl-Cは、子を待ち始める前に効かせる。ここで気付けないと、
        // 待機はこの子の終わりまで戻らない。
        if signal.interrupted() {
            terminate_child(child, spec);
            return Err(Error::Canceled);
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                // 直接の子が終わった後に残ったbyteだけを回収する。子孫がpipeを引き継いで
                // いてもEOFを待たず、読み取り端をここで閉じる。
                if let Err(error) = drain_pipe(&mut stdout, &mut stdout_bytes) {
                    return Err(unreadable(spec, &error.to_string()));
                }
                if let Err(error) = drain_pipe(&mut stderr, &mut stderr_bytes) {
                    return Err(unreadable(spec, &error.to_string()));
                }
                return Ok((status, stdout_bytes, stderr_bytes));
            }
            Ok(None) => {}
            Err(error) => {
                terminate_child(child, spec);
                return Err(spawn_failure(spec, &error));
            }
        }

        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            terminate_child(child, spec);
            return Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                msg!(
                    "error-external-command-timeout",
                    program = spec.program,
                    seconds = limit.map_or(0, |duration| duration.as_secs())
                ),
            ));
        }
    }
}

fn set_nonblocking<Fd: AsFd>(fd: &Fd) -> IoResult<()> {
    let flags = rustix::fs::fcntl_getfl(fd)?;
    Ok(rustix::fs::fcntl_setfl(
        fd,
        flags | rustix::fs::OFlags::NONBLOCK,
    )?)
}

fn drain_pipe<P: Read>(pipe: &mut Option<P>, collected: &mut Vec<u8>) -> IoResult<()> {
    let Some(reader) = pipe.as_mut() else {
        return Ok(());
    };
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                *pipe = None;
                return Ok(());
            }
            Ok(read) => collected.extend_from_slice(&buffer[..read]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

fn poll_pipes<O: AsFd, E: AsFd>(stdout: Option<&O>, stderr: Option<&E>) -> IoResult<(bool, bool)> {
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

fn unreadable(spec: &CommandSpec, cause: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandOutputUnreadable,
            msg!("error-external-command-output-unreadable"),
        )
        .fact(Fact::command(&spec.program))
        .fact(Fact::cause(cause)),
    )
}

#[cfg(test)]
#[path = "fake/run_capture_test.rs"]
mod run_capture_test;

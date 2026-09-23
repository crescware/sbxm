use std::io::Read;
use std::os::fd::AsFd;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, ExitStatus};
use std::time::{Duration, Instant};

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::{
    CommandSpec, InputFeed, SignalGuard, Stream, drain_pipe, poll_pipes, set_nonblocking,
    spawn_failure, terminate_child, unreadable, unwritable,
};

/// 直接の子が終わるまで2本のstreamを読み続け、届いたbyteを渡す。
///
/// pipeを読むthreadを残すと、子孫がpipeの書き込み端を引き継いだときにjoinが終わらない。
/// pipeをnonblockingにして子processの待機と同じloopで読むことで、子processが終わった時点で
/// 読み取り端を閉じられる。直接の子より長生きする子孫の出力は、その時点で捨てる。
///
/// 読んだbyteを溜めるか端末へ流すかは呼び出し側が決める。どちらであっても、読めなくなった
/// 相手をどう扱うか——まだ動いている子は終わらせてから報告し、既に終わった子の残りは
/// 諦める——は変わらない。受け手が受け取れないと答えた場合も、読めなくなった場合と同じく
/// 扱う。
pub(super) fn pump_until_exit(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    signal: Option<&SignalGuard>,
    receive: &mut dyn FnMut(Stream, &[u8]) -> std::io::Result<()>,
) -> Result<ExitStatus> {
    let (stdout, stderr) = take_pipes(child, spec)?;
    let input = take_input(child, spec)?;
    pump(child, spec, limit, signal, input, stdout, stderr, receive)
}

/// 渡すbyte列があれば、子の書き込み端を引き取り、待たずに書ける状態にする。
fn take_input<'a>(
    child: &mut Child,
    spec: &'a CommandSpec,
) -> Result<Option<InputFeed<'a, ChildStdin>>> {
    let Some(bytes) = spec.input() else {
        return Ok(None);
    };
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("the input pipe was not created"))
        .and_then(|stdin| set_nonblocking(&stdin).map(|()| stdin));
    match stdin {
        Ok(stdin) => Ok(Some(InputFeed::new(stdin, bytes))),
        Err(error) => {
            terminate_child(child);
            Err(unwritable(spec, &error.to_string()))
        }
    }
}

/// 子の読み取り端を引き取り、待たずに読める状態にする。
fn take_pipes(child: &mut Child, spec: &CommandSpec) -> Result<(ChildStdout, ChildStderr)> {
    let Some((stdout, stderr)) = child.stdout.take().zip(child.stderr.take()) else {
        terminate_child(child);
        return Err(unreadable(spec, "the output pipes were not created"));
    };
    if let Err(error) = set_nonblocking(&stdout).and_then(|()| set_nonblocking(&stderr)) {
        terminate_child(child);
        return Err(unreadable(spec, &error.to_string()));
    }
    Ok((stdout, stderr))
}

/// 読む相手を型で受け取り、pipeの生死をOSに委ねずに決められるようにする。
#[allow(clippy::too_many_arguments)]
fn pump<O: Read + AsFd, E: Read + AsFd, I: std::io::Write>(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    signal: Option<&SignalGuard>,
    mut input: Option<InputFeed<'_, I>>,
    stdout: O,
    stderr: E,
    receive: &mut dyn FnMut(Stream, &[u8]) -> std::io::Result<()>,
) -> Result<ExitStatus> {
    let mut stdout = Some(stdout);
    let mut stderr = Some(stderr);
    let deadline = limit.map(|duration| Instant::now() + duration);
    let interrupted = || signal.is_some_and(SignalGuard::interrupted);

    loop {
        if interrupted() {
            terminate_child(child);
            return Err(Error::Canceled);
        }

        // 子が読んだ分だけ書き足す。書き終えればstdinを閉じ、子へEOFを届ける。
        if let Some(feed) = input.as_mut()
            && let Err(error) = feed.feed()
        {
            terminate_child(child);
            return Err(unwritable(spec, &error.to_string()));
        }

        let (stdout_ready, stderr_ready) =
            poll_pipes(stdout.as_ref(), stderr.as_ref()).map_err(|error| {
                terminate_child(child);
                unreadable(spec, &error.to_string())
            })?;

        if stdout_ready && let Err(error) = read(&mut stdout, Stream::Stdout, receive) {
            terminate_child(child);
            return Err(unreadable(spec, &error.to_string()));
        }
        if stderr_ready && let Err(error) = read(&mut stderr, Stream::Stderr, receive) {
            terminate_child(child);
            return Err(unreadable(spec, &error.to_string()));
        }

        // 読んでいる間に届いたCtrl-Cは、子を待ち始める前に効かせる。ここで気付けないと、
        // 待機はこの子の終わりまで戻らない。
        if interrupted() {
            terminate_child(child);
            return Err(Error::Canceled);
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                // 直接の子が終わった後に残ったbyteだけを引き取る。子孫がpipeを引き継いで
                // いてもEOFを待たず、読み取り端をここで閉じる。
                read(&mut stdout, Stream::Stdout, receive)
                    .map_err(|error| unreadable(spec, &error.to_string()))?;
                read(&mut stderr, Stream::Stderr, receive)
                    .map_err(|error| unreadable(spec, &error.to_string()))?;
                return Ok(status);
            }
            Ok(None) => {}
            Err(error) => {
                terminate_child(child);
                return Err(spawn_failure(spec, &error));
            }
        }

        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            terminate_child(child);
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

fn read<P: Read>(
    pipe: &mut Option<P>,
    stream: Stream,
    receive: &mut dyn FnMut(Stream, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    drain_pipe(pipe, &mut |bytes| receive(stream, bytes))
}

#[cfg(test)]
#[path = "fake/pump_until_exit_test.rs"]
mod pump_until_exit_test;

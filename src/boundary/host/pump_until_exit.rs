use std::io::Read;
use std::process::ExitStatus;
use std::time::Duration;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::time::Clock;

use super::{
    CommandSpec, InputFeed, Pipes, Processes, Stream, drain_pipe, poll_pipes, spawn_failure,
    terminate_child, unreadable, unwritable,
};

/// 直接の子が終わるまで2本のstreamを読み続け、届いたbyteを渡す。
///
/// pipeを読むthreadを残すと、子孫がpipeの書き込み端を引き継いだときにjoinが終わらない。
/// pipeをnonblockingにして子processの待機と同じloopで読むことで、子processが終わった時点で
/// 読み取り端を閉じられる（契約test C7）。直接の子より長生きする子孫の出力は、その時点で捨てる。
///
/// 読んだbyteを溜めるか端末へ流すかは呼び出し側が決める。どちらであっても、読めなくなった
/// 相手をどう扱うか——まだ動いている子は終わらせてから報告し、既に終わった子の残りは
/// 諦める——は変わらない。受け手が受け取れないと答えた場合も、読めなくなった場合と同じく
/// 扱う。割り込まれたかは`interrupted`に尋ねる。Ctrl-Cを見張らない実行は、常に`false`と
/// 答える。
pub(super) fn pump_until_exit<O: Pipes>(
    os: &O,
    clock: &dyn Clock,
    child: &mut O::Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    interrupted: &dyn Fn() -> bool,
    receive: &mut dyn FnMut(Stream, &[u8]) -> std::io::Result<()>,
) -> Result<ExitStatus> {
    let (stdout, stderr) = take_pipes(os, child, spec)?;
    let input = take_input(os, child, spec)?;
    let ends = Ends {
        stdout: Some(stdout),
        stderr: Some(stderr),
        input,
    };
    pump(os, clock, child, spec, limit, interrupted, ends, receive)
}

/// まだ開いている端。
struct Ends<'a, O: Processes> {
    stdout: Option<O::Stdout>,
    stderr: Option<O::Stderr>,
    input: Option<InputFeed<'a, O::Stdin>>,
}

/// 渡すbyte列があれば、子の書き込み端を引き取り、待たずに書ける状態にする。
fn take_input<'a, O: Pipes>(
    os: &O,
    child: &mut O::Child,
    spec: &'a CommandSpec,
) -> Result<Option<InputFeed<'a, O::Stdin>>> {
    let Some(bytes) = spec.input() else {
        return Ok(None);
    };
    let Some(stdin) = os.take_stdin(child) else {
        terminate_child(os, child);
        return Err(unwritable(spec, "the input pipe was not created"));
    };
    if let Err(error) = os.stdin_nonblocking(&stdin) {
        terminate_child(os, child);
        return Err(unwritable(spec, &error.to_string()));
    }
    Ok(Some(InputFeed::new(stdin, bytes)))
}

/// 子の読み取り端を引き取り、待たずに読める状態にする。
fn take_pipes<O: Pipes>(
    os: &O,
    child: &mut O::Child,
    spec: &CommandSpec,
) -> Result<(O::Stdout, O::Stderr)> {
    let (Some(stdout), Some(stderr)) = (os.take_stdout(child), os.take_stderr(child)) else {
        terminate_child(os, child);
        return Err(unreadable(spec, "the output pipes were not created"));
    };
    if let Err(error) = os
        .stdout_nonblocking(&stdout)
        .and_then(|()| os.stderr_nonblocking(&stderr))
    {
        terminate_child(os, child);
        return Err(unreadable(spec, &error.to_string()));
    }
    Ok((stdout, stderr))
}

/// 端を開いたまま、子が終わるか、割り込まれるか、期限が来るまで回す。
#[allow(clippy::too_many_arguments)]
fn pump<O: Pipes>(
    os: &O,
    clock: &dyn Clock,
    child: &mut O::Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    interrupted: &dyn Fn() -> bool,
    mut ends: Ends<'_, O>,
    receive: &mut dyn FnMut(Stream, &[u8]) -> std::io::Result<()>,
) -> Result<ExitStatus> {
    let deadline = limit.map(|limit| (clock.now().after(limit), limit));
    loop {
        // 割り込みを見てから待つ。
        if interrupted() {
            terminate_child(os, child);
            return Err(Error::Canceled);
        }

        // 子が読んだ分だけ書き足す。書き終えればstdinを閉じ、子へEOFを届ける。
        if let Some(feed) = ends.input.as_mut()
            && let Err(error) = feed.feed()
        {
            terminate_child(os, child);
            return Err(unwritable(spec, &error.to_string()));
        }

        // 何かが届くか、書けるようになるまで短く待つ。
        let waiting = ends.input.as_ref().and_then(InputFeed::waiting);
        if let Err(error) = poll_pipes(os, ends.stdout.as_ref(), ends.stderr.as_ref(), waiting) {
            terminate_child(os, child);
            return Err(unreadable(spec, &error.to_string()));
        }

        // 待ちの結果に関わらず両方を読む。どちらも待たずに戻る。
        if let Err(error) = read(&mut ends.stdout, Stream::Stdout, receive) {
            terminate_child(os, child);
            return Err(unreadable(spec, &error.to_string()));
        }
        if let Err(error) = read(&mut ends.stderr, Stream::Stderr, receive) {
            terminate_child(os, child);
            return Err(unreadable(spec, &error.to_string()));
        }

        // 読んでいる間に届いたCtrl-Cは、子を待ち始める前に効かせる。ここで気付けないと、
        // 待機はこの子の終わりまで戻らない。
        if interrupted() {
            terminate_child(os, child);
            return Err(Error::Canceled);
        }

        match os.check_exit(child) {
            Ok(Some(status)) => {
                // 直接の子が終わった後に残ったbyteだけを引き取る。子孫がpipeを引き継いで
                // いてもEOFを待たず、読み取り端をここで閉じる。
                read(&mut ends.stdout, Stream::Stdout, receive)
                    .map_err(|error| unreadable(spec, &error.to_string()))?;
                read(&mut ends.stderr, Stream::Stderr, receive)
                    .map_err(|error| unreadable(spec, &error.to_string()))?;
                return Ok(status);
            }
            Ok(None) => {}
            Err(error) => {
                terminate_child(os, child);
                return Err(spawn_failure(spec, &error));
            }
        }

        if let Some((deadline, limit)) = deadline
            && clock.now() >= deadline
        {
            terminate_child(os, child);
            return Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                msg!(
                    "error-external-command-timeout",
                    program = spec.program,
                    seconds = limit.as_secs()
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
#[path = "pump_until_exit_test.rs"]
mod pump_until_exit_test;

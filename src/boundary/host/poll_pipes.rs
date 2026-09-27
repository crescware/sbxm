use std::io;

use super::{Pipes, WAIT_POLL_INTERVAL};

/// 開いている読み取り端と、書き残しのある書き込み端を、短く見張る。
///
/// どれが動いたかは返さない。どの端も待たずに読み書きできるため、次に試せば分かる。待ちは、
/// 何かが届いたときに早く戻るためだけにある。待っている間にsignal handlerが走ると、待ちは
/// `Interrupted`で終わる（契約test C2）。読めるものがあるかは次に読めば分かるため、何も
/// 無かったものとして続ける。
pub(super) fn poll_pipes<O: Pipes>(
    os: &O,
    stdout: Option<&O::Stdout>,
    stderr: Option<&O::Stderr>,
    input: Option<&O::Stdin>,
) -> io::Result<()> {
    let mut watched = Vec::with_capacity(3);
    if let Some(pipe) = stdout {
        watched.push(os.watch_stdout(pipe));
    }
    if let Some(pipe) = stderr {
        watched.push(os.watch_stderr(pipe));
    }
    if let Some(pipe) = input {
        watched.push(os.watch_stdin(pipe));
    }
    match os.wait_for_io(&mut watched, WAIT_POLL_INTERVAL) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(()),
        Err(error) => Err(error),
    }
}

use std::io;
use std::time::Duration;

use super::Processes;

/// 子とつながるpipeを待たずに読み書きできるようにし、どれかが動くまで短く待つ基本操作。
///
/// どの端が動いたかは返さない。どの端も待たずに読み書きできるため、次に試せば分かる。
pub(crate) trait Pipes: Processes {
    /// 1回の`wait_for_io`で見張る端。
    type Watch<'a>;

    /// stdinへ、待たずに書けるようにする。
    fn stdin_nonblocking(&self, pipe: &Self::Stdin) -> io::Result<()>;
    /// stdoutから、待たずに読めるようにする。
    fn stdout_nonblocking(&self, pipe: &Self::Stdout) -> io::Result<()>;
    /// stderrから、待たずに読めるようにする。
    fn stderr_nonblocking(&self, pipe: &Self::Stderr) -> io::Result<()>;
    /// 書けるようになるか、相手が閉じるのを見張る。
    fn watch_stdin<'a>(&self, pipe: &'a Self::Stdin) -> Self::Watch<'a>;
    /// 読めるようになるか、相手が閉じるのを見張る。
    fn watch_stdout<'a>(&self, pipe: &'a Self::Stdout) -> Self::Watch<'a>;
    /// 読めるようになるか、相手が閉じるのを見張る。
    fn watch_stderr<'a>(&self, pipe: &'a Self::Stderr) -> Self::Watch<'a>;
    /// 見張る端のどれかが動くか、`within`が過ぎるまで待つ。待っている間にsignal handlerが
    /// 走れば、`Interrupted`で戻る（契約test C2）。
    fn wait_for_io(&self, watched: &mut [Self::Watch<'_>], within: Duration) -> io::Result<usize>;
}

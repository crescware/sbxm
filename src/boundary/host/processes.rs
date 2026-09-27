use std::io::{self, Read, Write};
use std::process::{Command, ExitStatus};

/// 子processを起動し、終わりを尋ね、終わらせるための基本操作。
///
/// 判断のcodeは、いつ起動し、いつ尋ね、いつ終わらせるかだけを決める。実物はOS層の`RealHost`
/// であり、testは台本どおりに答えるものを渡す。methodの名前は、flakyになりうる要素の綴り
/// （`spawn`、`try_wait`、`wait`、`kill`）と重ねない。
pub(crate) trait Processes {
    /// 起動した直接の子。
    type Child;
    /// 子のstdinへの書き込み端。
    type Stdin: Write;
    /// 子のstdoutの読み取り端。
    type Stdout: Read;
    /// 子のstderrの読み取り端。
    type Stderr: Read;

    /// 次に起動する子を専用のprocess groupへ置く。端末からforeground groupへ届くCtrl-Cは、
    /// その子にも子孫にも届かない（契約test C12）。
    fn own_group(&self, command: &mut Command);
    /// 組み立てたcommandを起動する。
    fn start(&self, command: &mut Command) -> io::Result<Self::Child>;
    /// 待たずに、終わっていれば終了statusを返す。
    fn check_exit(&self, child: &mut Self::Child) -> io::Result<Option<ExitStatus>>;
    /// 終わるまで待ち、終了statusを引き取る。
    fn wait_exit(&self, child: &mut Self::Child) -> io::Result<ExitStatus>;
    /// 直接の子だけへ、直ちに終了signalを送る。
    fn end_child(&self, child: &mut Self::Child) -> io::Result<()>;
    /// 子のstdinを引き取る。pipeにしていなければ無い。
    fn take_stdin(&self, child: &mut Self::Child) -> Option<Self::Stdin>;
    /// 子のstdoutを引き取る。
    fn take_stdout(&self, child: &mut Self::Child) -> Option<Self::Stdout>;
    /// 子のstderrを引き取る。
    fn take_stderr(&self, child: &mut Self::Child) -> Option<Self::Stderr>;
}

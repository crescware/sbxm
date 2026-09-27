use std::io::Write;
use std::time::Duration;

use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{CommandOutcome, CommandSpec, PtyConfirmedCommand, TerminalCommand};

/// hostに対する外部commandの実行。testでは差し替える。
///
/// どの実行も、何をどの順で呼ぶかは`boundary::host`の判断のcodeが決め、OSの基本操作と時計だけを
/// 差し込みで受け取る。実際のhost（`RealHost`）はOS層の基本操作を、testのhostは答えを再生する
/// 基本操作を差し込む。
pub trait HostEnvironment {
    fn command_exists(&self, program: &str) -> bool;

    /// 出力をcaptureして実行する。
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome>;

    /// 出力が端末まで届くcommandを実行する。
    fn run_with_terminal(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
    ) -> Result<CommandOutcome>;

    /// 端末を引き渡したcommandを実行し、終わるのを待つあいだ`every`ごとに`tick`を呼ぶ。
    /// `tick`は端末へ何も書かない手続きとする。
    fn run_with_terminal_ticking(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
        every: Duration,
        tick: &mut dyn FnMut(),
    ) -> Result<CommandOutcome>;

    /// 出力をcaptureして実行し、stdoutだけを`sink`へ流す。`limit`byteを超えたら子を終わらせて拒否する。
    fn run_streaming(
        &self,
        spec: &CommandSpec,
        sink: &mut dyn Write,
        limit: u64,
    ) -> Result<CommandOutcome>;

    /// 確認promptにだけ答える、PTYの上でのcommand実行。利用者へ何も中継しない。
    fn run_pty_confirmed(&self, command: &PtyConfirmedCommand) -> Result<CommandOutcome>;
}

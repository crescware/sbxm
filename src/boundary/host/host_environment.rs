use std::io::Write;
use std::time::Duration;

use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{CommandOutcome, CommandSpec, Executable, TerminalCommand};

/// hostに対する外部commandの実行。testでは差し替える。
///
/// どの実行も、何をどの順で呼ぶかは`boundary::host`の判断のcodeが決め、OSの基本操作と時計だけを
/// 差し込みで受け取る。実際のhost（`RealHost`）はOS層の基本操作を、testのhostは答えを再生する
/// 基本操作を差し込む。
pub trait HostEnvironment {
    fn command_exists(&self, program: &str) -> bool;

    /// PATH上で`program`として起動されるfileの実体を、実行せずに観測する。
    ///
    /// 見つからない、または観測できない場合は`None`。`command_exists`が真でも、観測の
    /// あいだに差し替えられれば`None`になりうる。
    fn inspect_executable(&self, program: &str) -> Option<Executable>;

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
}

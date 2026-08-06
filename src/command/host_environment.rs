use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{CommandOutcome, CommandSpec, PtyConfirmedCommand, TerminalCommand};

/// hostに対する外部commandの実行。testでは差し替える。
pub trait HostEnvironment {
    fn command_exists(&self, program: &str) -> bool;

    /// 出力をcaptureして実行する。
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome>;

    /// 出力が端末まで届くcommandを実行する。
    ///
    /// 既定は`run`へ委ね、captureできた出力をまとめて渡す。実際のhostだけが、実行中の
    /// byteを届いた順に流す。testのhostは、何が端末まで届いたかをこの既定で観測できる。
    fn run_with_terminal(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
    ) -> Result<CommandOutcome> {
        let outcome = self.run(command.spec())?;
        output.relay(&outcome.stdout);
        output.relay(&outcome.stderr);
        output.finished();
        Ok(outcome)
    }

    /// 確認promptにだけ答える、PTYの上でのcommand実行。利用者へ何も中継しない。
    ///
    /// 既定は`run`へ委ね、答えたものとして進める。本物のPTYで期待するpromptを確認
    /// してから答えるのは、実際のhostだけである。
    fn run_pty_confirmed(&self, command: &PtyConfirmedCommand) -> Result<CommandOutcome> {
        self.run(&command.as_capture_spec())
    }
}

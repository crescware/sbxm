use crate::boundary::host::{CommandOutcome, CommandSpec, TerminalCommand};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::AnsweredHost;

/// 引数に`needle`を含む起動だけが、実行に至らずに終わるhost。ほかの起動は`inner`が答える。
///
/// `inner`の失敗する応答が「実行できて失敗した」を作るのに対し、こちらは起動そのものが
/// 成立しなかった場合を作る。実行に至らなかった起動は`inner`へ届かず、その記録にも残らない。
pub struct Unrunnable<H> {
    pub inner: H,
    needle: String,
    error: fn(&CommandSpec) -> Error,
}

impl<H> Unrunnable<H> {
    /// 応答が返らないまま時間切れになる起動。
    pub fn timing_out(inner: H, needle: &str) -> Unrunnable<H> {
        Unrunnable {
            inner,
            needle: needle.to_string(),
            error: timed_out,
        }
    }

    /// 利用者のCtrl-Cで中断された起動。`Error::Canceled`は診断を1件も持たない。
    pub fn canceled(inner: H, needle: &str) -> Unrunnable<H> {
        Unrunnable {
            inner,
            needle: needle.to_string(),
            error: |_| Error::Canceled,
        }
    }
}

impl<H: AnsweredHost> AnsweredHost for Unrunnable<H> {
    fn has_command(&self, program: &str) -> bool {
        self.inner.has_command(program)
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        if spec.args.join(" ").contains(&self.needle) {
            return Err((self.error)(spec));
        }
        self.inner.answer(spec)
    }

    fn session_length(
        &self,
        command: &TerminalCommand,
        every: std::time::Duration,
    ) -> std::time::Duration {
        self.inner.session_length(command, every)
    }
}

fn timed_out(spec: &CommandSpec) -> Error {
    Error::new(
        ErrorId::ExternalCommandTimeout,
        msg!(
            "error-external-command-timeout",
            program = spec.program,
            seconds = 10
        ),
    )
}

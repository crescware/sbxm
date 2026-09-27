use std::cell::{Cell, RefCell};

use crate::boundary::host::{CommandOutcome, CommandSpec, TerminalCommand};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::AnsweredHost;

/// `at`番目（0から数える）の起動だけが失敗するhost。ほかの起動は`inner`が答える。
///
/// 工程の途中の起動を1つずつ失敗させ、どの起動の失敗も成功として扱われないことを
/// 確かめるために使う。`at`を指定しなければ、失敗させずに起動を記録するだけである。
pub struct FailingAt<H> {
    pub inner: H,
    at: Option<usize>,
    timing_out: bool,
    seen: Cell<usize>,
    calls: RefCell<Vec<String>>,
}

impl<H> FailingAt<H> {
    /// 失敗させずに、起動を記録する。
    pub fn recording(inner: H) -> FailingAt<H> {
        FailingAt {
            inner,
            at: None,
            timing_out: false,
            seen: Cell::new(0),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// `at`番目の起動が、実行できて失敗する。
    pub fn exiting(inner: H, at: usize) -> FailingAt<H> {
        FailingAt {
            at: Some(at),
            ..FailingAt::recording(inner)
        }
    }

    /// `at`番目の起動が、応答を返さないまま時間切れになる。
    pub fn timing_out(inner: H, at: usize) -> FailingAt<H> {
        FailingAt {
            at: Some(at),
            timing_out: true,
            ..FailingAt::recording(inner)
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl<H: AnsweredHost> AnsweredHost for FailingAt<H> {
    fn has_command(&self, program: &str) -> bool {
        self.inner.has_command(program)
    }

    fn session_length(
        &self,
        command: &TerminalCommand,
        every: std::time::Duration,
    ) -> std::time::Duration {
        self.inner.session_length(command, every)
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let index = self.seen.get();
        self.seen.set(index + 1);
        self.calls
            .borrow_mut()
            .push(format!("{} {}", spec.program, spec.args.join(" ")));
        if self.at != Some(index) {
            return self.inner.answer(spec);
        }
        if self.timing_out {
            return Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                msg!(
                    "error-external-command-timeout",
                    program = spec.program,
                    seconds = 10
                ),
            ));
        }
        Ok(crate::testing::command::outcome(spec, 1, ""))
    }
}

use std::cell::RefCell;
use std::time::Duration;

use crate::boundary::host::{CommandOutcome, CommandSpec, TerminalCommand};
use crate::diagnostics::Result;

use super::AnsweredHost;

/// 引数に`needle`を含む最初の起動の直前に、1回だけ`action`を走らせるhost。答えは`inner`が
/// 返す。
///
/// 工程の途中でhostの外の状態が変わった場合を、その工程のあいだに作るために使う。
pub struct ChangingBefore<H> {
    pub inner: H,
    needle: String,
    action: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl<H> ChangingBefore<H> {
    pub fn new(inner: H, needle: &str, action: impl FnOnce() + 'static) -> ChangingBefore<H> {
        ChangingBefore {
            inner,
            needle: needle.to_string(),
            action: RefCell::new(Some(Box::new(action))),
        }
    }
}

impl<H: AnsweredHost> AnsweredHost for ChangingBefore<H> {
    fn has_command(&self, program: &str) -> bool {
        self.inner.has_command(program)
    }

    fn session_length(&self, command: &TerminalCommand, every: Duration) -> Duration {
        self.inner.session_length(command, every)
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        if spec.args.join(" ").contains(&self.needle)
            && let Some(action) = self.action.borrow_mut().take()
        {
            action();
        }
        self.inner.answer(spec)
    }
}

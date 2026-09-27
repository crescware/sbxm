use std::cell::{Cell, RefCell};
use std::time::Duration;

use crate::boundary::host::{CommandOutcome, CommandSpec, TerminalCommand};
use crate::diagnostics::Result;

use super::AnsweredHost;

/// 引数に`needle`を含む起動のうち、`skip`件を通したあとの起動の直前に、1回だけ`action`を
/// 走らせるhost。答えは`inner`が返す。
///
/// 工程の途中でhostの外の状態が変わった場合を、その工程のあいだに作るために使う。
pub struct ChangingBefore<H> {
    pub inner: H,
    needle: String,
    skip: Cell<usize>,
    action: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl<H> ChangingBefore<H> {
    /// 最初に一致する起動の直前に走らせる。
    pub fn new(inner: H, needle: &str, action: impl FnOnce() + 'static) -> ChangingBefore<H> {
        ChangingBefore::skipping(inner, needle, 0, action)
    }

    /// 一致する起動を`skip`件通し、その次の起動の直前に走らせる。
    pub fn skipping(
        inner: H,
        needle: &str,
        skip: usize,
        action: impl FnOnce() + 'static,
    ) -> ChangingBefore<H> {
        ChangingBefore {
            inner,
            needle: needle.to_string(),
            skip: Cell::new(skip),
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
        if spec.args.join(" ").contains(&self.needle) {
            match self.skip.get() {
                0 => {
                    if let Some(action) = self.action.borrow_mut().take() {
                        action();
                    }
                }
                skip => self.skip.set(skip - 1),
            }
        }
        self.inner.answer(spec)
    }
}

use std::cell::{Cell, RefCell};
use std::time::Duration;

use crate::time::{Clock, Moment};

/// 待つたびに、待った分だけ進む時計。
///
/// 実時間を待たずに期限へ達する。何をどれだけ待ったかは`slept`に残る。
#[derive(Debug, Default)]
pub struct ScriptedClock {
    now: Cell<Duration>,
    pub slept: RefCell<Vec<Duration>>,
}

impl ScriptedClock {
    /// 時計を`duration`だけ進める。待つあいだに時間が経ったことを表す。
    pub fn advance(&self, duration: Duration) {
        self.now.set(self.now.get() + duration);
    }
}

impl Clock for ScriptedClock {
    fn now(&self) -> Moment {
        Moment::since_origin(self.now.get())
    }

    fn sleep(&self, duration: Duration) {
        self.slept.borrow_mut().push(duration);
        self.advance(duration);
    }
}

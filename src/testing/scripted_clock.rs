use std::cell::{Cell, RefCell};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::time::{Clock, Moment};

/// 壁時計の起点。UNIX epochから数えて`20260923T101501Z`にあたる。
const WALL_ORIGIN: Duration = Duration::from_secs(1_790_158_501);

/// 待つたびに、待った分だけ進む時計。
///
/// 実時間を待たずに期限へ達する。何をどれだけ待ったかは`slept`に残る。壁時計は
/// `WALL_ORIGIN`から始まり、読みと同じだけ進む。
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

    /// 時計を`at`まで進める。既に過ぎていれば動かさない。
    pub fn advance_to(&self, at: Duration) {
        self.now.set(self.now.get().max(at));
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

    fn wall(&self) -> SystemTime {
        UNIX_EPOCH + WALL_ORIGIN + self.now.get()
    }
}

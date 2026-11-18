use std::sync::LazyLock;
use std::time::{Duration, Instant, SystemTime};

use crate::time::{Clock, Moment};

/// processの時計の起点。最初に読んだ時点を0とする。
static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);

/// OSによる経過時間の計測、threadを止める待機、現在の日時の取得。
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Moment {
        Moment::since_origin(ORIGIN.elapsed())
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn wall(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[cfg(test)]
#[path = "system_clock_test.rs"]
mod system_clock_test;

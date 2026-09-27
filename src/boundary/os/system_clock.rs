use std::sync::LazyLock;
use std::time::{Duration, Instant};

use crate::time::{Clock, Moment};

/// processの時計の起点。最初に読んだ時点を0とする。
static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);

/// OSの単調な時計と、threadを止める待ち。
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Moment {
        Moment::since_origin(ORIGIN.elapsed())
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
#[path = "system_clock_test.rs"]
mod system_clock_test;

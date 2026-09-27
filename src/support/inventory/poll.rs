use std::time::Duration;

use crate::time::{Clock, Moment};

/// 状態が変わるのを待つ間隔と上限、それを数える時計。
///
/// 起動と停止の完了は、commandの戻り値ではなく一覧のstateを読み直して判定する。
#[derive(Clone, Copy)]
pub struct Poll<'a> {
    pub interval: Duration,
    pub limit: Duration,
    pub clock: &'a dyn Clock,
}

impl<'a> Poll<'a> {
    /// 2秒ごとに読み直し、60秒で諦める。
    pub fn standard(clock: &'a dyn Clock) -> Poll<'a> {
        Poll {
            interval: Duration::from_secs(2),
            limit: Duration::from_secs(60),
            clock,
        }
    }

    /// 今から待ち始めたときの期限。
    pub fn deadline(self) -> Moment {
        self.clock.now().after(self.limit)
    }

    /// `deadline`に届いたか。
    pub fn expired(self, deadline: Moment) -> bool {
        self.clock.now() >= deadline
    }

    /// 次に読み直すまで待つ。
    pub fn pause(self) {
        self.clock.sleep(self.interval);
    }
}

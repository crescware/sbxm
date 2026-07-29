//! testが状態の変化を待つ間隔。

use crate::workflow::inventory::Poll;
use std::time::Duration;

/// 実時間を待たずに待機の打ち切りまで進める間隔と上限。
pub fn poll() -> Poll {
    Poll {
        interval: Duration::from_millis(1),
        limit: Duration::from_millis(20),
    }
}

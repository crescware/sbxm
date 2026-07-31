use std::time::Duration;

/// 状態が変わるのを待つ間隔と上限。
///
/// 起動と停止の完了は、commandの戻り値ではなく一覧のstateを読み直して判定する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Poll {
    pub interval: Duration,
    pub limit: Duration,
}

impl Default for Poll {
    fn default() -> Poll {
        Poll {
            interval: Duration::from_secs(2),
            limit: Duration::from_secs(60),
        }
    }
}

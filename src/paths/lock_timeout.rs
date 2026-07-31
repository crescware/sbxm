use std::time::Duration;

/// lock取得の待機上限。
pub const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// macOSの`sysctl -n kern.waketime`から、最後の復帰時刻を読む。
/// 日付の表示部分はlocaleに依るため使わず、timevalの数値だけを検証する。
pub fn parse_wake_time(output: &str) -> Option<SystemTime> {
    let (fields, _) = output.trim().strip_prefix('{')?.split_once('}')?;
    let (seconds, micros) = fields.split_once(',')?;
    let seconds = field(seconds, "sec")?;
    let micros = field(micros, "usec")?;
    if seconds == 0 || micros >= 1_000_000 {
        return None;
    }
    UNIX_EPOCH.checked_add(Duration::from_secs(seconds).checked_add(Duration::from_micros(micros))?)
}

fn field(output: &str, name: &str) -> Option<u64> {
    let (key, value) = output.trim().split_once('=')?;
    let value = value.trim();
    if key.trim() != name || value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

use crate::boundary::host::protocol::RootDiskUsage;
use crate::design::{CapacityMeter, Field, Inline, VisualState};
use crate::msg;

use super::format_capacity;

/// 観測を残量表示へ変える。予約領域や`df`の丸め済み使用率は分母に使わない。
///
/// 全体ゼロなど割合を求められない観測は、空きゼロという表示へ丸めず未確認にする。
pub fn disk_meter(usage: &RootDiskUsage) -> Option<CapacityMeter> {
    if usage.free_kib > usage.usable_kib {
        return None;
    }
    let percent = (u128::from(usage.free_kib) * 100).checked_div(u128::from(usage.usable_kib))?;
    let percent = u8::try_from(percent).ok()?;
    let remaining = if percent == 0 && usage.free_kib > 0 {
        "<1%".to_string()
    } else {
        format!("{percent}%")
    };
    // 絶対量の境界を先に判定する。100 MiBちょうどは「空きなし」に含めない。
    let (state, status) = if usage.free_kib < 100 * 1024 {
        (VisualState::Negative, Some(msg!("status-disk-no-space")))
    } else if percent <= 10 {
        (VisualState::Negative, Some(msg!("status-disk-very-low")))
    } else if percent <= 25 {
        (VisualState::Attention, Some(msg!("status-disk-low")))
    } else {
        (VisualState::Positive, None)
    };

    Some(CapacityMeter {
        target: Inline::path("/"),
        available: Field::new(
            msg!("status-disk-available"),
            Inline::text(format_capacity(usage.free_kib)),
        ),
        remaining: msg!("status-disk-remaining", percent = remaining),
        percent,
        state,
        status,
        total: Field::new(
            msg!("status-disk-total"),
            Inline::text(format_capacity(usage.usable_kib)),
        ),
    })
}

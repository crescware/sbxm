use crate::boundary::host::protocol::RootDiskUsage;
use crate::design::VisualState;
use crate::i18n::{Catalog, Locale};
use crate::testing::outcome::{Checked, Required};

use super::*;

const MIB: u64 = 1024;
const GIB: u64 = 1024 * MIB;

fn usage(free_kib: u64, usable_kib: u64) -> RootDiskUsage {
    RootDiskUsage {
        free_kib,
        usable_kib,
        capacity_percent: 99,
    }
}

#[test]
fn the_unit_depends_on_absolute_capacity_and_never_on_the_percentage() {
    for (kib, expected) in [
        (0, "0 MiB"),
        (1, "<1 MiB"),
        (MIB - 1, "<1 MiB"),
        (MIB, "1 MiB"),
        (100 * MIB - 1, "99 MiB"),
        (100 * MIB, "100 MiB"),
        (100 * MIB + 1, "100 MiB"),
        (GIB - 1, "1023 MiB"),
        (GIB, "1.0 GiB"),
        (GIB + GIB / 2, "1.5 GiB"),
        (4_898_320, "4.7 GiB"),
        (19_401_296, "18.5 GiB"),
    ] {
        assert_eq!(format_capacity(kib), expected, "{kib} KiB");
    }
    assert!(format_capacity(u64::MAX).ends_with(" GiB"));
}

#[test]
fn remaining_is_computed_before_rounding_and_ignores_dfs_usage_percentage() -> Checked {
    for total in [2 * GIB, 100 * GIB] {
        let meter = disk_meter(&usage(512 * MIB, total)).required()?;
        assert_eq!(meter.available.value.as_str(), "512 MiB");
    }
    let meter = disk_meter(&usage(4_898_320, 19_401_296)).required()?;
    assert_eq!(meter.percent, 25);
    assert_eq!(meter.available.value.as_str(), "4.7 GiB");
    assert_eq!(meter.total.value.as_str(), "18.5 GiB");
    assert_eq!(
        Catalog::new(Locale::En)
            .format(&meter.remaining)
            .required()?,
        " (25% remaining)"
    );
    Ok(())
}

#[test]
fn no_space_is_an_absolute_strictly_less_than_100_mib_boundary() -> Checked {
    for free in [0, 1, 96 * MIB, 100 * MIB - 1, 100 * MIB, 100 * MIB + 1] {
        let meter = disk_meter(&usage(free, 20 * GIB)).required()?;
        assert_eq!(meter.state, VisualState::Negative);
        assert_eq!(
            meter.status.as_ref().map(|status| status.id),
            Some(if free < 100 * MIB {
                "status-disk-no-space"
            } else {
                "status-disk-very-low"
            })
        );
        let text = Catalog::new(Locale::En)
            .format(&meter.remaining)
            .required()?;
        assert_eq!(
            text,
            if free == 0 {
                " (0% remaining)"
            } else {
                " (<1% remaining)"
            }
        );
    }
    // 小さいfilesystemでは割合が大きくても、絶対量の警告を優先する。
    let meter = disk_meter(&usage(99 * MIB, 100 * MIB)).required()?;
    assert_eq!(meter.percent, 99);
    assert_eq!(
        meter.status.as_ref().map(|status| status.id),
        Some("status-disk-no-space")
    );
    Ok(())
}

#[test]
fn warning_colors_and_labels_agree_with_the_displayed_whole_percentage() -> Checked {
    for (tenths, percent, state, status) in [
        (10, 1, VisualState::Negative, Some("status-disk-very-low")),
        (100, 10, VisualState::Negative, Some("status-disk-very-low")),
        (109, 10, VisualState::Negative, Some("status-disk-very-low")),
        (110, 11, VisualState::Attention, Some("status-disk-low")),
        (250, 25, VisualState::Attention, Some("status-disk-low")),
        (259, 25, VisualState::Attention, Some("status-disk-low")),
        (260, 26, VisualState::Positive, None),
        (430, 43, VisualState::Positive, None),
        (1000, 100, VisualState::Positive, None),
    ] {
        let meter = disk_meter(&usage(tenths * 100 * MIB, 100_000 * MIB)).required()?;
        assert_eq!(meter.percent, percent);
        assert_eq!(meter.state, state);
        assert_eq!(meter.status.as_ref().map(|status| status.id), status);
    }
    Ok(())
}

#[test]
fn invalid_totals_are_unobserved_and_large_values_do_not_overflow() -> Checked {
    assert!(disk_meter(&usage(0, 0)).is_none());
    assert!(disk_meter(&usage(2, 1)).is_none());
    let meter = disk_meter(&usage(u64::MAX, u64::MAX)).required()?;
    assert_eq!(meter.percent, 100);
    assert_eq!(meter.state, VisualState::Positive);
    Ok(())
}

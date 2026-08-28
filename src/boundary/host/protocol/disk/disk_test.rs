use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

#[test]
fn the_root_line_is_read_by_its_trailing_mount_point() -> Checked {
    // 実測形。予約領域が5.2%あるため`Size - Used != Available`。
    let observed = "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay          20466256  14502976   4898320       75% /\n";
    assert_eq!(
        parse_df(observed).required()?,
        RootDiskUsage {
            free_kib: 4_898_320,
            usable_kib: 14_502_976 + 4_898_320,
            capacity_percent: 75,
        }
    );
    Ok(())
}

#[test]
fn other_filesystems_are_skipped_in_favor_of_the_line_that_mounts_root() -> Checked {
    let observed = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n/dev/sda1          1000000    500000    500000       50% /var/lib/docker\noverlay          20466256  14502976   4898320       75% /\n";
    assert_eq!(parse_df(observed).required()?.capacity_percent, 75);
    Ok(())
}

#[test]
fn crlf_line_endings_and_extra_whitespace_do_not_change_the_reading() -> Checked {
    let observed = "Filesystem     1024-blocks      Used Available Capacity Mounted on\r\noverlay   20466256   14502976    4898320   75%   /\r\n";
    assert_eq!(
        parse_df(observed).required()?,
        RootDiskUsage {
            free_kib: 4_898_320,
            usable_kib: 19_401_296,
            capacity_percent: 75,
        }
    );
    Ok(())
}

#[test]
fn free_is_available_not_size_minus_used() -> Checked {
    // Size(20466256) - Used(14502976) = 5963280 != Available(4898320)。5.2%の予約領域。
    let observed = "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay          20466256  14502976   4898320       75% /\n";
    let usage = parse_df(observed).required()?;
    assert_eq!(usage.free_kib, 4_898_320);
    assert_ne!(usage.free_kib, 20_466_256 - 14_502_976);
    assert_eq!(usage.usable_kib, usage.free_kib + 14_502_976);
    Ok(())
}

#[test]
fn a_missing_root_line_a_short_row_and_malformed_fields_are_all_refused() -> Checked {
    let cases = [
        "",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\n",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay 20466256 14502976 75% /\n",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay 20466256 not-a-number 4898320 75% /\n",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay 20466256 14502976 not-a-number 75% /\n",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay 20466256 14502976 4898320 75 /\n",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay 20466256 14502976 4898320 101% /\n",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay 20466256 14502976 4898320 -1% /\n",
    ];
    for output in cases {
        let error = parse_df(output)
            .refused_because(&format!("malformed output is rejected: {output:?}"))?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
    Ok(())
}

#[test]
fn used_plus_available_overflowing_a_u64_is_refused() -> Checked {
    let observed = format!(
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay {} {} {} 75% /\n",
        u64::MAX,
        u64::MAX,
        u64::MAX
    );
    let error = parse_df(&observed).refused_because("an overflowing sum is never wrapped")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    Ok(())
}

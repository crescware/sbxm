use super::RootDiskUsage;
use crate::diagnostics::{Result, unparseable};

/// 1 filesystem行のblock counterを厳密に読む。
pub(super) fn parse_df_row(line: &str, program: &str) -> Result<RootDiskUsage> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let [_filesystem, _size, used, available, capacity, _mount] = fields.as_slice() else {
        return Err(unparseable(
            program,
            &format!("expected 6 columns, found {}: {line}", fields.len()),
        ));
    };

    let used: u64 = used
        .parse()
        .map_err(|_| unparseable(program, &format!("{used} is not a whole number of KiB")))?;
    let available: u64 = available.parse().map_err(|_| {
        unparseable(
            program,
            &format!("{available} is not a whole number of KiB"),
        )
    })?;
    let usable_kib = used
        .checked_add(available)
        .ok_or_else(|| unparseable(program, "used + available overflows a 64-bit KiB count"))?;

    let percent = capacity
        .strip_suffix('%')
        .ok_or_else(|| unparseable(program, &format!("{capacity} is not a percentage")))?;
    let capacity_percent: u8 = percent
        .parse()
        .ok()
        .filter(|value| *value <= 100)
        .ok_or_else(|| unparseable(program, &format!("{capacity} is not 0-100%")))?;

    Ok(RootDiskUsage {
        free_kib: available,
        usable_kib,
        capacity_percent,
    })
}

use crate::diagnostics::{Result, unparseable};

use super::RootDiskUsage;

/// commandの表示に使う、翻訳しないprogram名。
const PROGRAM: &str = "df -Pk /";

/// `df -Pk /`の出力をparseする。
///
/// POSIX形式の見出し行1行と、`/`をmountするfilesystemの行1行を読む。`Size`列は
/// 利用者向けの空き容量計算に使わない。overflow、percentの書式違反、列不足は
/// いずれも`Err`とし、`0`や`Reachable`のような安全な値へ丸めない。
pub fn parse_df(output: &str) -> Result<RootDiskUsage> {
    let line = output
        .lines()
        .skip(1)
        .map(|line| line.trim_end_matches('\r'))
        .find(|line| line.split_whitespace().next_back() == Some("/"))
        .ok_or_else(|| unparseable(PROGRAM, "no line reports the root filesystem \"/\""))?;

    let fields: Vec<&str> = line.split_whitespace().collect();
    let [_filesystem, _size, used, available, capacity, _mount] = fields.as_slice() else {
        return Err(unparseable(
            PROGRAM,
            &format!("expected 6 columns, found {}: {line}", fields.len()),
        ));
    };

    let used: u64 = used
        .parse()
        .map_err(|_| unparseable(PROGRAM, &format!("{used} is not a whole number of KiB")))?;
    let available: u64 = available.parse().map_err(|_| {
        unparseable(
            PROGRAM,
            &format!("{available} is not a whole number of KiB"),
        )
    })?;
    let usable_kib = used
        .checked_add(available)
        .ok_or_else(|| unparseable(PROGRAM, "used + available overflows a 64-bit KiB count"))?;

    let percent = capacity
        .strip_suffix('%')
        .ok_or_else(|| unparseable(PROGRAM, &format!("{capacity} is not a percentage")))?;
    let capacity_percent: u8 = percent
        .parse()
        .ok()
        .filter(|value| *value <= 100)
        .ok_or_else(|| unparseable(PROGRAM, &format!("{capacity} is not 0-100%")))?;

    Ok(RootDiskUsage {
        free_kib: available,
        usable_kib,
        capacity_percent,
    })
}

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

    super::parse_df_row::parse_df_row(line, PROGRAM)
}

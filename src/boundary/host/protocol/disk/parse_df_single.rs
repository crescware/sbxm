use super::RootDiskUsage;
use crate::diagnostics::{Result, unparseable};

/// `df -Pk <path>`が返す1 filesystem行。mountがrootとは限らない。
pub fn parse_df_single(output: &str) -> Result<RootDiskUsage> {
    let mut rows = output
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty());
    let row = rows
        .next()
        .ok_or_else(|| unparseable("df -Pk", "missing filesystem row"))?;
    if rows.next().is_some() {
        return Err(unparseable("df -Pk", "ambiguous filesystem rows"));
    }
    super::parse_df_row::parse_df_row(row, "df -Pk")
}

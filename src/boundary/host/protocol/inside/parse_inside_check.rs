use std::time::Duration;

use crate::diagnostics::{Result, unparseable};

use super::InsideRecord;

/// commandの表示に使う、翻訳しないprogram名。
const PROGRAM: &str = "sbx exec";

/// `INSIDE_CHECK`の出力を、渡したcommandの数だけの結果へ読む。
///
/// 見出しの番号が順どおりでない、byte数が足りない、結果の数が合わない、余りがある、の
/// いずれも`Err`とする。読めた分だけを答えとして返すと、途中で止まった手順を、後ろの
/// commandが無かったかのように読むことになる。
pub fn parse_inside_check(output: &[u8], expected: usize) -> Result<Vec<InsideRecord>> {
    let mut records = Vec::with_capacity(expected);
    let mut rest = output;
    for index in 0..expected {
        let Some(end) = rest.iter().position(|byte| *byte == b'\n') else {
            return Err(unparseable(
                PROGRAM,
                &format!("record {index} of {expected} has no header line"),
            ));
        };
        let header = String::from_utf8_lossy(&rest[..end]).into_owned();
        rest = &rest[end + 1..];
        let fields: Vec<&str> = header.split(' ').collect();
        let [number, status, stdout_len, stderr_len, elapsed] = fields.as_slice() else {
            return Err(unparseable(
                PROGRAM,
                &format!(
                    "record {index}: expected 5 header fields, found {}: {header}",
                    fields.len()
                ),
            ));
        };
        if number.parse::<usize>().ok() != Some(index) {
            return Err(unparseable(
                PROGRAM,
                &format!("record {index}: the header names record {number}"),
            ));
        }
        let status: i32 = status
            .parse()
            .ok()
            .filter(|status| (0..=255).contains(status))
            .ok_or_else(|| {
                unparseable(
                    PROGRAM,
                    &format!("record {index}: {status} is not an exit status"),
                )
            })?;
        let elapsed = parse_elapsed(elapsed, index)?;
        let stdout = take(&mut rest, stdout_len, index, "stdout")?;
        let stderr = take(&mut rest, stderr_len, index, "stderr")?;
        records.push(InsideRecord {
            status,
            stdout,
            stderr,
            elapsed,
        });
    }
    if !rest.is_empty() {
        return Err(unparseable(
            PROGRAM,
            &format!("{} bytes follow the last of {expected} records", rest.len()),
        ));
    }
    Ok(records)
}

/// 見出しの所要時間。`-`は計らなかったことを示し、それ以外はnano秒の整数である。
fn parse_elapsed(field: &str, index: usize) -> Result<Option<Duration>> {
    if field == "-" {
        return Ok(None);
    }
    field
        .parse::<u64>()
        .map(|nanos| Some(Duration::from_nanos(nanos)))
        .map_err(|_| {
            unparseable(
                PROGRAM,
                &format!("record {index}: {field} is not an elapsed time in nanoseconds"),
            )
        })
}

/// 見出しが告げた長さのbyte列を切り出す。
fn take(rest: &mut &[u8], length: &str, index: usize, stream: &str) -> Result<Vec<u8>> {
    let length: usize = length.parse().map_err(|_| {
        unparseable(
            PROGRAM,
            &format!("record {index}: {length} is not a byte count for {stream}"),
        )
    })?;
    if rest.len() < length {
        return Err(unparseable(
            PROGRAM,
            &format!(
                "record {index}: {stream} announces {length} bytes but {} remain",
                rest.len()
            ),
        ));
    }
    let (taken, remaining) = rest.split_at(length);
    *rest = remaining;
    Ok(taken.to_vec())
}

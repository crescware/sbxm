use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::diagnostics::Result;
use crate::msg;

use super::{BLOCK, MAX_ENTRY_BYTES, entry_name, octal, unreadable, unusable};

// `SeekFrom::Current`へ渡すsignedなblock幅。tarのblock幅は固定512 byte。
const BLOCK_OFFSET: i64 = 512;

/// tar archiveから1件のentryを取り出す。
///
/// entry本体を読み飛ばしながらheaderだけを辿るため、archiveの大きさに依存しない。
pub(super) fn read_entry(path: &Path, wanted: &str) -> Result<Option<Vec<u8>>> {
    let mut file = File::open(path).map_err(|error| unreadable(path, None, &error.to_string()))?;

    loop {
        let mut header = [0_u8; BLOCK];
        match file.read_exact(&mut header) {
            Ok(()) => {}
            // 末尾に達した。要求されたentryは存在しない。
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => {
                return Err(unreadable(path, None, &error.to_string()));
            }
        }
        // 終端はNULで埋めたblockが並ぶ。
        if header.iter().all(|byte| *byte == 0) {
            return Ok(None);
        }

        let name = entry_name(&header)
            .ok_or_else(|| unusable(path, msg!("cause-archive-entry-unnamed")))?;
        let size = octal(&header[124..136]).ok_or_else(|| {
            unusable(
                path,
                msg!("cause-archive-entry-size-unreadable", entry = name),
            )
        })?;

        if name == wanted {
            // metadataとして読める大きさは、宣言した上限とこの環境で確保できる大きさの
            // 両方へ収まるものである。2つの判定へ分けると、同じ報告を2度書いたうえで、
            // 上限を先に確かめる限り後の分岐は起こらなくなる。
            let capacity = match usize::try_from(size) {
                Ok(capacity) if size <= MAX_ENTRY_BYTES => capacity,
                _ => {
                    return Err(unusable(
                        path,
                        msg!(
                            "cause-archive-entry-too-large",
                            entry = name,
                            observed = size
                        ),
                    ));
                }
            };
            let mut data = vec![0_u8; capacity];
            file.read_exact(&mut data)
                .map_err(|error| unreadable(path, Some(&name), &error.to_string()))?;
            return Ok(Some(data));
        }

        // entry本体は512 byte単位で詰められている。
        let padded =
            (size / BLOCK_OFFSET + i64::from(u8::from(size % BLOCK_OFFSET != 0))) * BLOCK_OFFSET;
        file.seek(SeekFrom::Current(padded))
            .map_err(|error| unreadable(path, None, &error.to_string()))?;
    }
}

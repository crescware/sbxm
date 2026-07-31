use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::diagnostics::Result;

use super::{BLOCK, MAX_ENTRY_BYTES, entry_name, octal, unusable};

/// tar archiveから1件のentryを取り出す。
///
/// entry本体を読み飛ばしながらheaderだけを辿るため、archiveの大きさに依存しない。
pub(super) fn read_entry(path: &Path, wanted: &str) -> Result<Option<Vec<u8>>> {
    let mut file = File::open(path)
        .map_err(|error| unusable(path, &format!("the archive could not be opened: {error}")))?;

    loop {
        let mut header = [0_u8; BLOCK];
        match file.read_exact(&mut header) {
            Ok(()) => {}
            // 末尾に達した。要求されたentryは存在しない。
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => {
                return Err(unusable(
                    path,
                    &format!("the archive could not be read: {error}"),
                ));
            }
        }
        // 終端はNULで埋めたblockが並ぶ。
        if header.iter().all(|byte| *byte == 0) {
            return Ok(None);
        }

        let name =
            entry_name(&header).ok_or_else(|| unusable(path, "an entry has no readable name"))?;
        let size = octal(&header[124..136])
            .ok_or_else(|| unusable(path, &format!("entry {name} has no readable size")))?;

        if name == wanted {
            if size > MAX_ENTRY_BYTES {
                return Err(unusable(
                    path,
                    &format!("{name} is {size} bytes, which is larger than sbxm reads"),
                ));
            }
            let capacity = usize::try_from(size).map_err(|_| {
                unusable(
                    path,
                    &format!("{name} is {size} bytes, which is larger than sbxm reads"),
                )
            })?;
            let mut data = vec![0_u8; capacity];
            file.read_exact(&mut data)
                .map_err(|error| unusable(path, &format!("{name} could not be read: {error}")))?;
            return Ok(Some(data));
        }

        // entry本体は512 byte単位で詰められている。
        let padded = size.div_ceil(BLOCK as u64) * BLOCK as u64;
        let padded = i64::try_from(padded)
            .map_err(|_| unusable(path, &format!("entry {name} declares an unusable size")))?;
        file.seek(SeekFrom::Current(padded)).map_err(|error| {
            unusable(path, &format!("the archive could not be scanned: {error}"))
        })?;
    }
}

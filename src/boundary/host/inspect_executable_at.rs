use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use super::{Executable, ExecutableIdentity};

/// scriptの先頭の2byte。`#`と`!`。
const SHEBANG: [u8; 2] = [b'#', b'!'];

/// 起動されるfileの実体を観測する。
///
/// symlinkを解決し、実体のmetadataと先頭の2byteを読む。実体が通常fileでない、または
/// 読めない場合は、観測できなかったとして`None`を返す。推測で埋めない。
pub(super) fn inspect_executable_at(path: &Path) -> Option<Executable> {
    let resolved = fs::canonicalize(path).ok()?;
    let metadata = fs::metadata(&resolved).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let mut head = [0u8; 2];
    let read = File::open(&resolved).ok()?.read(&mut head).ok()?;
    Some(Executable {
        identity: ExecutableIdentity {
            path: resolved,
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.size(),
            mtime_secs: metadata.mtime(),
            mtime_nanos: metadata.mtime_nsec(),
            ctime_secs: metadata.ctime(),
            ctime_nanos: metadata.ctime_nsec(),
        },
        script: read == head.len() && head == SHEBANG,
    })
}

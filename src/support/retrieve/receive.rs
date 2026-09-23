use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::paths::{self, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope};

use crate::support::sandbox;

/// Sandbox内の`args`がstdoutへ書いたbyte列を、hostの`directory`へ`name`として受け取る。
///
/// `directory`は利用者のfileを置かないprivateな隔離領域とする。一時fileへ書いてから
/// renameし、失敗したときは書きかけのfileを残さない。同じ名前のfileが既にあれば、
/// 上書きせず拒否する。受け取るのは`limit`byteまでとする。
pub fn receive(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
    directory: &Path,
    name: &str,
    limit: u64,
    timeout: TimeoutClass,
) -> Result<PathBuf> {
    paths::ensure_private_dir(directory, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    let target = directory.join(name);
    // 一時fileは、成功してrenameするまでdropで消える。
    let mut temporary = private_temporary(directory)
        .map_err(|error| paths::atomic_write_failed(&target, &error.to_string()))?;
    sandbox::exec_streaming(host, sandbox, args, temporary.as_file_mut(), limit, timeout)?
        .require_success()?;
    keep(temporary, &target)
        .map_err(|error| paths::atomic_write_failed(&target, &error.to_string()))?;
    Ok(target)
}

/// 本人だけが読み書きできる一時file。
fn private_temporary(directory: &Path) -> std::io::Result<NamedTempFile> {
    let temporary = tempfile::Builder::new()
        .prefix(".receiving-")
        .tempfile_in(directory)?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
    Ok(temporary)
}

/// 書き終えた一時fileを、既存のfileを上書きせずに`target`へ移す。
fn keep(temporary: NamedTempFile, target: &Path) -> std::io::Result<()> {
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(target)
        .map_err(|error| error.error)?;
    if let Some(parent) = target.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }
    Ok(())
}

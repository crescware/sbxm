use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

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
    let failed =
        |error: &dyn std::fmt::Display| paths::atomic_write_failed(&target, &error.to_string());

    // 一時fileは、成功してrenameするまでdropで消える。
    let mut temporary = tempfile::Builder::new()
        .prefix(".receiving-")
        .tempfile_in(directory)
        .map_err(|error| failed(&error))?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))
        .map_err(|error| failed(&error))?;

    let mut writer = std::io::BufWriter::new(temporary.as_file_mut());
    sandbox::exec_streaming(host, sandbox, args, &mut writer, limit, timeout)?.require_success()?;
    writer.flush().map_err(|error| failed(&error))?;
    drop(writer);
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| failed(&error))?;

    temporary
        .persist_noclobber(&target)
        .map_err(|error| failed(&error.error))?;
    if let Ok(parent) = File::open(directory) {
        let _ = parent.sync_all();
    }
    Ok(target)
}

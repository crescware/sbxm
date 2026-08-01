use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use crate::paths::inspect::display;

use super::temp_path_for;

/// atomic writeの共通部分。
///
/// 1. 同一directoryに`create_new`で一時fileを作る
/// 2. 必要permissionを設定する
/// 3. 全内容を書いて`sync_all`する
/// 4. 呼び出し側のprecondition検査を通す
/// 5. renameする
/// 6. 親directoryを`sync_all`する
pub(super) fn atomic_write_with_precondition(
    target: &Path,
    contents: &str,
    mode: u32,
    precondition: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let temp = temp_path_for(target)?;
    let write_failed = |detail: String| {
        Error::single(
            Diagnostic::new(
                ErrorId::AtomicWriteFailed,
                msg!("error-atomic-write-failed"),
            )
            .fact(Fact::path(&display(target)))
            .fact(Fact::cause(&detail)),
        )
    };

    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temp)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::TempFileLeftBehind,
                    msg!("error-temp-file-left-behind", path = display(&temp)),
                )
                .remediation(msg!("remediation-remove-temp-file", path = display(&temp))),
            ));
        }
        Err(error) => return Err(write_failed(error.to_string())),
    };

    let result = (|| -> Result<()> {
        // umaskの影響を受けずに、要求したpermissionを確定させる。
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|error| write_failed(error.to_string()))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| write_failed(error.to_string()))?;
        file.sync_all()
            .map_err(|error| write_failed(error.to_string()))?;
        precondition(target)?;
        fs::rename(&temp, target).map_err(|error| write_failed(error.to_string()))?;
        if let Some(parent) = target.parent()
            && let Ok(directory) = File::open(parent)
        {
            // rename自体を永続化する。失敗しても書き込み内容は失われないため、致命的には扱わない。
            let _ = directory.sync_all();
        }
        Ok(())
    })();

    if result.is_err() {
        // rename前の失敗では、この実行が作った一時fileだけを片付ける。
        let _ = fs::remove_file(&temp);
    }
    result
}

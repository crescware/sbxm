use std::fs::{self, Permissions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::InitialProvisioningFile;
use crate::msg;
use crate::paths;
use crate::support::files;

use super::Pulled;

/// 取り出したSandbox側の内容で、hostの宣言fileを置き換える。
///
/// 差分を見せたあとでhostの宣言fileが変わっていれば、見せたものと違う置き換えになるため
/// 拒否する。置き換えは同じdirectoryの一時fileからのrenameで行い、元のpermissionを保つ。
/// 置き換えたあとは、hostとSandboxが同じ内容になったことをこの案件のbaselineへ記録する。
pub fn adopt(pulled: &mut Pulled) -> Result<PathBuf> {
    let source = pulled.declaration.source.as_path().to_path_buf();
    if files::read_source(&source)? != pulled.host_sha256 {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileUnusable,
                msg!("error-declared-file-unusable"),
            )
            .fact(Fact::source(&paths::display(&source)))
            .fact(Fact::reason(msg!("cause-host-file-changed-while-deciding"))),
        ));
    }
    let recorded = InitialProvisioningFile {
        source: paths::display(&source),
        destination: paths::display(pulled.declaration.destination.as_path()),
        sha256: pulled.copy.sha256.clone(),
    };
    files::PendingUpdate::begin(
        &pulled.locked.paths,
        &recorded,
        Some(pulled.host_sha256.clone()),
        true,
    )?;
    replace(&source, &pulled.copy.path)
        .map_err(|error| paths::atomic_write_failed(&source, &error.to_string()))?;
    paths::sync_parent_directory(&source)?;
    files::PendingUpdate::recover_host(&pulled.locked.paths, &mut pulled.locked.metadata)?;
    Ok(source)
}

/// `target`を`received`の内容で置き換える。元のpermissionを保ち、renameで入れ替える。
fn replace(target: &Path, received: &Path) -> std::io::Result<()> {
    let bytes = fs::read(received)?;
    let permissions: Permissions = fs::metadata(target)?.permissions();
    let parent = target
        .parent()
        .ok_or_else(|| std::io::Error::other("the file has no parent directory"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".sbxm-adopting-")
        .tempfile_in(parent)?;
    temporary.as_file().set_permissions(permissions)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(target).map_err(|error| error.error)?;
    Ok(())
}

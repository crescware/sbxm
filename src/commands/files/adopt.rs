use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, InitialProvisioningFile};
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
    let changed = |reason| {
        Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileUnusable,
                msg!("error-declared-file-unusable"),
            )
            .fact(Fact::source(&paths::display(&source)))
            .fact(Fact::reason(reason)),
        )
    };
    if files::read_source(&source)? != pulled.host_sha256 {
        return Err(changed(msg!("cause-host-file-changed-while-deciding")));
    }
    let failed =
        |error: &dyn std::fmt::Display| paths::atomic_write_failed(&source, &error.to_string());
    let bytes = fs::read(&pulled.copy.path).map_err(|error| failed(&error))?;
    let permissions = fs::metadata(&source)
        .map_err(|error| failed(&error))?
        .permissions();
    let parent = source
        .parent()
        .ok_or_else(|| failed(&"the file has no parent directory"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".sbxm-adopting-")
        .tempfile_in(parent)
        .map_err(|error| failed(&error))?;
    temporary
        .as_file()
        .set_permissions(permissions)
        .map_err(|error| failed(&error))?;
    temporary
        .write_all(&bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| failed(&error))?;
    temporary
        .persist(&source)
        .map_err(|error| failed(&error.error))?;

    let recorded = InitialProvisioningFile {
        source: paths::display(&source),
        destination: paths::display(pulled.declaration.destination.as_path()),
        sha256: pulled.copy.sha256.clone(),
    };
    let mut baseline = pulled
        .locked
        .metadata
        .declared_files
        .clone()
        .unwrap_or_default();
    baseline.retain(|entry| {
        crate::config::SandboxHomeRelativePath::new(&entry.destination).map_or(true, |placed| {
            !placed.names_same_place(&pulled.declaration.destination)
        })
    });
    baseline.push(recorded);
    pulled.locked.metadata.declared_files = Some(baseline);
    metadata::update(&pulled.locked.paths, &pulled.locked.metadata)?;
    Ok(source)
}

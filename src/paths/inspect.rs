//! pathの観測。
//!
//! symlinkを追跡せず、所有者とpermissionは観測した値で判定する。ここにあるものは
//! filesystemを変更しない。

use std::fs::{self, File};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use crate::error::{Error, ErrorId, Result};
use crate::msg;

use super::scope::PathScope;

/// 表示用のpath文字列。非UTF-8 pathもlossyに表示する。
pub fn display(path: &Path) -> String {
    path.display().to_string()
}

/// symlinkを追跡せず、`.`と`..`をlexicalに解決する。
///
/// filesystemを参照しないため、存在しないpathにも適用できる。
pub fn lexically_standardize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

/// symlinkを解決できない場合は宣言されたpathのまま比較する。
pub fn real_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| lexically_standardize(path))
}

/// pathがsymlinkかどうか。存在しない場合は`false`。
pub fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

/// open済みfileと、pathが指す実体が同一かを判定するためのidentity。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
}

impl FileIdentity {
    pub fn of_open_file(file: &File) -> std::io::Result<FileIdentity> {
        let metadata = file.metadata()?;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    /// symlinkを追跡せずpathのidentityを取る。
    pub fn of_path_without_following(path: &Path) -> std::io::Result<FileIdentity> {
        let metadata = fs::symlink_metadata(path)?;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

/// group・otherに権限が残っているか。
pub fn permission_too_open(mode: u32) -> bool {
    mode & 0o077 != 0
}

/// modeを`0o600`のような表示にする。
pub fn format_mode(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}

/// このprocessの実効user ID。
///
/// permissionだけでは、ほかのaccountが所有する`0700`のdirectoryを自分のものと
/// 区別できない。所有関係は観測した値で判定する。
pub(super) fn current_user() -> u32 {
    // SAFETY: geteuid(2)は引数を取らず、失敗しない。
    unsafe { libc::geteuid() }
}

/// 現在の利用者が所有していないpathを、内容を変更せず拒否する。
pub(super) fn require_owned_by_current_user(
    path: &Path,
    observed: u32,
    scope: PathScope,
) -> Result<()> {
    let expected = current_user();
    if observed == expected {
        return Ok(());
    }
    Err(scope.owner_error(path, observed, expected))
}

/// pathが通常fileとして存在するかを、symlinkを追跡せずに判定する。
///
/// symlink、directory、特殊fileは、内容を変更せず拒否する。
pub fn regular_file_exists(path: &Path, scope: PathScope) -> Result<bool> {
    if is_symlink(path) {
        return Err(scope.symlink_error(path));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(metadata) => Err(unexpected_type(path, "regular file", &metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(scope.unreadable_error(path, &error.to_string())),
    }
}

/// 開いたfileが、現在の利用者だけが読み書きできる通常fileであることを確認する。
pub(super) fn require_private_file(
    file: &File,
    path: &Path,
    mode: u32,
    scope: PathScope,
) -> Result<()> {
    let metadata = file
        .metadata()
        .map_err(|error| scope.unreadable_error(path, &error.to_string()))?;
    if !metadata.is_file() {
        return Err(unexpected_type(path, "regular file", &metadata));
    }
    require_owned_by_current_user(path, metadata.uid(), scope)?;
    let observed = metadata.permissions().mode();
    if permission_too_open(observed) {
        return Err(scope.permission_error(path, observed, mode));
    }
    Ok(())
}

/// 既存pathのfile type。診断で観測値として示す。翻訳しない技術表記。
fn file_type_of(metadata: &fs::Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_dir() {
        "directory"
    } else if file_type.is_file() {
        "regular file"
    } else if file_type.is_symlink() {
        "symbolic link"
    } else {
        "special file"
    }
}

/// 期待するfile typeと異なるpathを、内容を変更せず拒否する。
pub(super) fn unexpected_type(
    path: &Path,
    expected: &'static str,
    metadata: &fs::Metadata,
) -> Error {
    Error::new(
        ErrorId::ProjectPathUnexpectedType,
        msg!(
            "error-project-path-unexpected-type",
            path = display(path),
            expected = expected,
            observed = file_type_of(metadata)
        ),
    )
}

#[cfg(test)]
#[path = "inspect_test.rs"]
mod inspect_test;

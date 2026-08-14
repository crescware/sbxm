use std::path::Path;

use crate::design::{Fact, Warning};
use crate::diagnostics::Result;
use crate::hash::SHORT_HEX_LENGTH;
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};

/// project lockを保持している間に、前回のcrash等で残った短命archiveを片付ける。
///
/// `template-<hex>.tar`または`template-<hex>.tar.tmp`という厳密な名前のregular file
/// だけを対象にする。symlink、directory、特殊file、名前が一致しないfileは追跡も削除
/// もしない。`.cache`が無い場合は何もしない。`.cache`自体がsymlinkの場合はその先を
/// 辿らず拒否する。観測または削除できなかった対象は、無いものとして扱わずwarningで
/// 示す。何度実行しても結果は変わらない。
pub fn cleanup_stale_archives(paths: &ProjectPaths) -> Result<Vec<Warning>> {
    let cache_dir = paths.cache_dir();
    // `read_dir`はsymlinkの先を辿る。`.cache`自体が別directoryへのsymlinkに差し替え
    // られていた場合、その先の名前が一致するfileを削除してしまう。先に検査する。
    if paths::is_symlink(&cache_dir) {
        return Err(PathScope::ProjectPath.symlink_error(&cache_dir));
    }

    // 列挙の途中で1件でも観測できなければ、それ以後を安全に追跡できる保証もない。
    // `collect`はiteratorの最初のErrで打ち切る。次回のstale cleanupが続きを拾う。
    let entries = match std::fs::read_dir(&cache_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Ok(with_unreadable_warning(Vec::new(), &cache_dir, &error)),
    };
    let entries: Vec<std::fs::DirEntry> = match entries.collect() {
        Ok(entries) => entries,
        Err(error) => return Ok(with_unreadable_warning(Vec::new(), &cache_dir, &error)),
    };

    let mut warnings = Vec::new();
    for entry in entries {
        let path = entry.path();
        if !is_stale_archive_name(&path) {
            continue;
        }
        // `symlink_metadata`はsymlinkの先を辿らない。symlinkとdirectoryはどちらも
        // `is_file()`が`false`になり、対象から外れる。
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Ok(with_unreadable_warning(warnings, &path, &error)),
        };
        if !metadata.is_file() {
            continue;
        }
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            warnings.push(cleanup_failed_warning(&path, &error));
        }
    }
    Ok(warnings)
}

/// 観測できなかった1件をwarningとして積み、それまでに集めた分と一緒に返す。
fn with_unreadable_warning(
    mut warnings: Vec<Warning>,
    path: &Path,
    error: &std::io::Error,
) -> Vec<Warning> {
    warnings.push(unreadable_warning(path, error));
    warnings
}

fn unreadable_warning(path: &Path, error: &std::io::Error) -> Warning {
    Warning::text(msg!("warning-stale-archive-unreadable"))
        .fact(Fact::path(&paths::display(path)))
        .fact(Fact::cause(&error.to_string()))
}

fn cleanup_failed_warning(path: &Path, error: &std::io::Error) -> Warning {
    Warning::text(msg!("warning-stale-archive-cleanup-failed"))
        .fact(Fact::path(&paths::display(path)))
        .fact(Fact::cause(&error.to_string()))
}

/// `template-<12桁の16進数小文字>.tar`、または同じhexに`.tar.tmp`が付いた名前か。
fn is_stale_archive_name(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(hex) = name.strip_prefix("template-") else {
        return false;
    };
    let hex = match hex.strip_suffix(".tar.tmp") {
        Some(hex) => hex,
        None => match hex.strip_suffix(".tar") {
            Some(hex) => hex,
            None => return false,
        },
    };
    hex.len() == SHORT_HEX_LENGTH
        && hex
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

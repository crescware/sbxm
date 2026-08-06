use std::path::Path;

use crate::hash::SHORT_HEX_LENGTH;
use crate::paths::ProjectPaths;

/// project lockを保持している間に、前回のcrash等で残った短命archiveを片付ける。
///
/// `template-<hex>.tar`または`template-<hex>.tar.tmp`という厳密な名前のregular file
/// だけを対象にする。symlink、directory、特殊file、名前が一致しないfileは追跡も削除
/// もしない。`.cache`が無い、または読めない場合は何もしない。何度実行しても結果は
/// 変わらない。
pub fn cleanup_stale_archives(paths: &ProjectPaths) {
    let Ok(entries) = std::fs::read_dir(paths.cache_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_stale_archive_name(&path) {
            continue;
        }
        // `symlink_metadata`はsymlinkの先を辿らない。symlinkとdirectoryはどちらも
        // `is_file()`が`false`になり、対象から外れる。
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let _ = std::fs::remove_file(&path);
    }
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

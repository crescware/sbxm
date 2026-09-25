use std::fs;
use std::path::Path;

use crate::diagnostics::Result;
use crate::paths;

/// `directory`のbundleを、新しいものから`keep`件だけ残して消す。
///
/// bundleは受け取った順に並べる。名前は受け取った時刻と、同じ秒に重なったときの
/// 番号から成る。bundle以外のfileには触れない。
pub fn prune_bundles(directory: &Path, keep: usize) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(paths::atomic_write_failed(directory, &error.to_string())),
    };
    let mut bundles: Vec<_> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "bundle")
        })
        .collect();
    bundles.sort_by_cached_key(|path| arrival(path));
    let stale = bundles.len().saturating_sub(keep);
    for path in bundles.iter().take(stale) {
        fs::remove_file(path)
            .map_err(|error| paths::atomic_write_failed(path, &error.to_string()))?;
    }
    Ok(())
}

/// bundleを受け取った順の並べ方。`<時刻>-<番号>`は、番号の無い`<時刻>`の後に番号の
/// 順で来る。名前の文字順では`-2`が`<時刻>`より、`-10`が`-2`より前に来てしまう。
fn arrival(path: &Path) -> (String, u64) {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    match stem.rsplit_once('-') {
        Some((stamp, attempt)) => match attempt.parse() {
            Ok(attempt) => (stamp.to_string(), attempt),
            Err(_) => (stem, 1),
        },
        None => (stem, 1),
    }
}

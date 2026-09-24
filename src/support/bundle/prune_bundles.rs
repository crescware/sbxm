use std::fs;
use std::path::Path;

use crate::diagnostics::Result;
use crate::paths;

/// `directory`のbundleを、新しいものから`keep`件だけ残して消す。
///
/// bundleの名前は受け取った時刻の順に並ぶ。bundle以外のfileには触れない。
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
    bundles.sort();
    let stale = bundles.len().saturating_sub(keep);
    for path in bundles.iter().take(stale) {
        fs::remove_file(path)
            .map_err(|error| paths::atomic_write_failed(path, &error.to_string()))?;
    }
    Ok(())
}

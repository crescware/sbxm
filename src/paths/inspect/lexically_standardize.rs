use std::path::{Component, Path, PathBuf};

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

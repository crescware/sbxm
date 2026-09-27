use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

/// 親を含めてdirectoryを作る。permissionはmkdirの時点で`mode`に決める。
///
/// umaskはbitを落とすだけで足さないため、作った時点で`mode`より広いことはない。
pub(super) fn create_private_directory(path: &Path, mode: u32) -> std::io::Result<()> {
    DirBuilder::new().recursive(true).mode(mode).create(path)
}

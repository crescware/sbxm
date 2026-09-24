use std::fs;
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::support::retrieve;

use super::{CREATE_BUNDLE, MAX_BUNDLE_BYTES, ReceivedBundle};

/// Sandboxのbare repositoryを、hostの`directory`へbundleとして受け取る。
///
/// file名は`stamp`から作る。同じ名前が既にあれば番号を足し、既存のbundleを上書き
/// しない。Sandboxが保存するrefを1つも持たなければ`None`を返す。
pub fn receive_bundle(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    directory: &Path,
    stamp: &str,
) -> Result<Option<ReceivedBundle>> {
    let mut label = stamp.to_string();
    let mut attempt = 1;
    while directory.join(format!("{label}.bundle")).exists() {
        attempt += 1;
        label = format!("{stamp}-{attempt}");
    }
    let path = retrieve::receive(
        host,
        sandbox,
        &["sh", "-c", CREATE_BUNDLE, "sh", git_dir],
        directory,
        &format!("{label}.bundle"),
        MAX_BUNDLE_BYTES,
        TimeoutClass::RepositoryTransfer,
    )?;
    // 何も書かれなかったbundleは、保存するものが無かったことを表す。
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() == 0) {
        let _ = fs::remove_file(&path);
        return Ok(None);
    }
    Ok(Some(ReceivedBundle { path, label }))
}

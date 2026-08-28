use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::{ImageIdentity, parse_image_inspect};
use crate::diagnostics::Result;
use crate::support::docker;

/// imageの現在の同一性。存在しない場合は`None`。
///
/// `docker image inspect`は不在でも他の失敗でも非ゼロで終わるため、それだけで
/// 不在と判定しない。まず一覧で存在を確かめ、observeできない状態はerrorとして返す。
pub fn inspect(host: &dyn HostEnvironment, name: &str) -> Result<Option<ImageIdentity>> {
    if !docker::exists(host, name)? {
        return Ok(None);
    }
    let outcome = docker::inspect(host, name)?;
    parse_image_inspect(&outcome.stdout_text())
        .map(Some)
        .map_err(|error| docker::diagnose_failure(host, error))
}

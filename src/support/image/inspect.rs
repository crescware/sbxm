use crate::command::{CommandSpec, HostEnvironment, TimeoutClass};
use crate::compatibility::{ImageIdentity, parse_image_inspect};
use crate::diagnostics::Result;

use super::exists;

/// imageの現在の同一性。存在しない場合は`None`。
///
/// `docker image inspect`は不在でも他の失敗でも非ゼロで終わるため、それだけで
/// 不在と判定しない。まず一覧で存在を確かめ、observeできない状態はerrorとして返す。
pub fn inspect(host: &dyn HostEnvironment, name: &str) -> Result<Option<ImageIdentity>> {
    if !exists(host, name)? {
        return Ok(None);
    }
    let spec = CommandSpec::capture("docker", &["image", "inspect", name])
        .timeout(TimeoutClass::LocalFilesystem);
    let outcome = host.run(&spec)?.require_success()?;
    parse_image_inspect(&outcome.stdout_text()).map(Some)
}

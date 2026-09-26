use crate::boundary::host::HostEnvironment;
use crate::diagnostics::{Result, unparseable};
use crate::support::sandbox;

use super::PLACE_SAVE_REFS;

/// 保存の前に、Sandboxの`git_dir`へworktreeの`HEAD`を一時refとして置く。
///
/// 保存するrefがあるかを答える。何も書かない答えも、`ready`でも`empty`でもない答えも、
/// 保存するものが無いとは読まない。
pub(super) fn prepare_save(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
) -> Result<bool> {
    let answer = sandbox::exec(host, sandbox, &["sh", "-c", PLACE_SAVE_REFS, "sh", git_dir])?
        .require_success()?
        .stdout_text();
    match answer.trim() {
        "ready" => Ok(true),
        "empty" => Ok(false),
        _ => Err(unparseable(
            "sbx exec",
            "placing the worktree heads answered neither ready nor empty",
        )),
    }
}

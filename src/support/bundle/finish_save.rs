use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::support::sandbox;

use super::CLEAR_SAVE_REFS;

/// 保存のあとに、Sandboxの`git_dir`から一時refを消す。
pub(super) fn finish_save(host: &dyn HostEnvironment, sandbox: &str, git_dir: &str) -> Result<()> {
    sandbox::exec(host, sandbox, &["sh", "-c", CLEAR_SAVE_REFS, "sh", git_dir])?
        .require_success()?;
    Ok(())
}

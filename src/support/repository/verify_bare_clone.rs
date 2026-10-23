use crate::boundary::host::HostEnvironment;
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;

use crate::support::sandbox::{self, Live};

use super::{SandboxOrigin, unusable, verify_bare_config};

/// 既存のbare repositoryが案件のrepositoryとして再利用できるかを観測する。
///
/// 設定の確認（`verify_bare_config`）に、objectの接続性の確認を加える。repositoryを
/// 作るときと、hostと同期するときに使う。`open`が毎回行う観測は設定の確認だけである。
pub fn verify_bare_clone(
    host: &dyn HostEnvironment,
    sandbox: &str,
    origin: &SandboxOrigin,
    git_dir: &str,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    verify_bare_config(&Live::new(host, sandbox), origin, git_dir, progress)?;

    progress.step(msg!("progress-inspect-repository-connectivity"));
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "--git-dir", git_dir, "fsck", "--connectivity-only"],
    )?;
    if !outcome.success() {
        return Err(unusable(git_dir, msg!("cause-connectivity-check-failed")));
    }
    Ok(())
}

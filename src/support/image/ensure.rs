use std::path::Path;

use crate::command::HostEnvironment;
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::project::{CanonicalProjectId, SandboxName};

use super::{BuiltImage, ensure_verified, verify_generation};

/// 世代に対応するimageを用意する。
///
/// 既存imageは、全labelが一致した場合だけ再利用する。世代名を先に確認するため、
/// 別の案件や別の世代が同じ名前を持っていた場合は何も作らずに停止する。
pub fn ensure(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    canonical: &CanonicalProjectId,
    dockerfile: &Path,
    dockerfile_sha256: &str,
    progress: &mut dyn ProgressSink,
) -> Result<BuiltImage> {
    let verified = verify_generation(host, sandbox, canonical, dockerfile_sha256)?;
    ensure_verified(
        host,
        sandbox,
        canonical,
        dockerfile,
        dockerfile_sha256,
        verified,
        progress,
    )
}

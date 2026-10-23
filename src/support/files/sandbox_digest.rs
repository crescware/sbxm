use std::path::Path;

use crate::diagnostics::Result;

use crate::support::sandbox::Inside;

use super::{AGENT_HOME, destination_path, digest_in_sandbox, require_no_symlink_in_sandbox};

/// 宣言fileの配置先が、Sandboxで今持っている内容のdigest。無ければ`None`。
///
/// 配置と同じく、配置先までの途中にsymbolic linkがあれば読まずに拒否する。何も変更しない。
pub fn sandbox_digest(
    inside: &dyn Inside,
    source: &Path,
    destination: &Path,
) -> Result<Option<String>> {
    let destination = destination_path(destination)?;
    require_no_symlink_in_sandbox(inside, source, &destination)?;
    digest_in_sandbox(inside, &format!("{AGENT_HOME}/{destination}"))
}

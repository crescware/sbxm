use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::paths;

use crate::support::sandbox;

use super::AGENT_HOME;

/// destinationが`agent` homeからsymlinkを経ずに届くことを確かめる。
///
/// 配置はroot権限で行うため、途中のcomponentがsymlinkであれば、read、chown、
/// 置き換えのいずれもhomeの外へ及ぶ。homeに近い側から1階層ずつ確認する。
pub(super) fn require_no_symlink_in_sandbox(
    host: &dyn HostEnvironment,
    sandbox: &str,
    source: &Path,
    destination: &str,
) -> Result<()> {
    let mut current = AGENT_HOME.to_string();
    for part in destination.split('/') {
        current.push('/');
        current.push_str(part);
        if sandbox::exec(host, sandbox, &["test", "-h", &current])?.success() {
            return Err(Error::new(
                ErrorId::DeclaredFileUnusable,
                msg!(
                    "error-declared-file-unusable",
                    source = paths::display(source),
                    detail = format!("{current} is a symbolic link inside the sandbox")
                ),
            ));
        }
    }
    Ok(())
}

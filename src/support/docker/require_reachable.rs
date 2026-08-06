use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::version_probe;

/// Docker Engineへ疎通できることを確認する。
pub fn require_reachable(host: &dyn HostEnvironment) -> Result<()> {
    let outcome = version_probe(host)?;
    if outcome.success() && !outcome.stdout_text().trim().is_empty() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(ErrorId::DockerUnreachable, msg!("error-docker-unreachable"))
            .fact(Fact::reason(msg!("cause-server-version-unreadable")))
            .remediation(msg!("remediation-start-docker")),
    ))
}

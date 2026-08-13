use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::version_probe;

/// Docker Engineへ疎通できることを確認する。
pub fn require_reachable(host: &dyn HostEnvironment) -> Result<()> {
    let outcome = match version_probe(host) {
        Ok(outcome) => outcome,
        Err(error @ Error::Canceled) => return Err(error),
        Err(error) => return Err(unreachable(Some(&error))),
    };
    if outcome.success() && !outcome.stdout_text().trim().is_empty() {
        return Ok(());
    }
    Err(unreachable(None))
}

/// probeが実行できなかった場合も、Dockerが使えないという診断へ揃える。
///
/// timeoutやspawn failureをそのまま返すと、利用者にはDocker Desktopを再起動する
/// remediationが届かない。元のerror IDと外部failureは、診断の事実として残す。
fn unreachable(source: Option<&Error>) -> Error {
    let mut diagnostic =
        Diagnostic::new(ErrorId::DockerUnreachable, msg!("error-docker-unreachable"))
            .fact(Fact::reason(msg!("cause-server-version-unreadable")))
            .remediation(msg!("remediation-start-docker"));
    if let Some(source) = source.and_then(|error| error.diagnostics().first()) {
        let cause = source.id.to_string();
        diagnostic = diagnostic.fact(Fact::cause(&cause));
        if let Some(external) = &source.external {
            diagnostic = diagnostic.external(external.clone());
        }
    }
    Error::single(diagnostic)
}

#[cfg(test)]
#[path = "require_reachable_test.rs"]
mod require_reachable_test;

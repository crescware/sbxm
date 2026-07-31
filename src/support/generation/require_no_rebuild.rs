use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;

/// 世代の切替中は工程を進めず、`rebuild`の完了を案内する。
pub fn require_no_rebuild(metadata: &ProjectMetadata) -> Result<()> {
    if metadata.rebuild.is_none() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RebuildIntentPending,
            msg!(
                "error-rebuild-intent-pending",
                project = metadata.display_id()
            ),
        )
        .remediation(
            Remediation::text(msg!("remediation-run-rebuild"))
                .try_run(format!("sbxm rebuild {}", metadata.display_id())),
        ),
    ))
}

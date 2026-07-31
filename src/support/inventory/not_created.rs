use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::metadata::ProjectMetadata;
use crate::msg;

use crate::design::Remediation;

/// Sandboxをまだ持たない案件を、構築commandとともに拒否する。
pub fn not_created(metadata: &ProjectMetadata, sandbox: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxNotCreated,
            msg!(
                "error-sandbox-not-created",
                project = metadata.display_id(),
                sandbox = sandbox
            ),
        )
        // 案件は既に登録済みである。`add`はimageにもsandboxにも触れないため、
        // 構築するcommandを案内する。
        .remediation(
            Remediation::text(msg!("remediation-sandbox-not-created"))
                .try_run(format!("sbxm prepare {}", metadata.display_id())),
        ),
    )
}

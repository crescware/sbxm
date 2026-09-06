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
        // 構築するcommandを案内する。初回構築は`open`が同じ実行の中で行うため、
        // 構築だけを行う別commandへは送らない。
        .remediation(
            Remediation::text(msg!("remediation-sandbox-not-created"))
                .try_run(format!("sbxm open {}", metadata.display_id())),
        ),
    )
}

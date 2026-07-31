use crate::command::HostEnvironment;
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{GITHUB_HOST, GITHUB_TOKEN_ENV, placeholder_probe};

/// placeholderがSandboxへ届いていることを、中から確かめる。
///
/// custom secretはSandboxの作成時に結び付く。登録済みという事実から届いたと推定せず、
/// 環境変数を観測する。値は判定にも表示にも使わず、空かどうかだけを見る。
pub fn require_placeholder_present(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let outcome =
        crate::support::sandbox::exec(host, sandbox, &["sh", "-c", &placeholder_probe()])?
            .require_success()?;
    if !outcome.stdout_text().trim().is_empty() {
        return Ok(());
    }

    Err(Error::single(
        Diagnostic::new(
            ErrorId::SandboxSecretNotApplied,
            msg!(
                "error-sandbox-secret-not-applied",
                sandbox = sandbox,
                env = GITHUB_TOKEN_ENV,
                host = GITHUB_HOST
            ),
        )
        .remediation(
            Remediation::text(msg!("remediation-sandbox-secret-not-applied"))
                .try_run(format!("sbx rm {sandbox}")),
        ),
    ))
}

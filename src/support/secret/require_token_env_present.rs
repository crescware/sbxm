use crate::boundary::host::HostEnvironment;
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{GITHUB_HOST, GITHUB_TOKEN_ENV, placeholder_probe, register_command};

/// `GH_TOKEN`がSandboxへ届いていることを、中から確かめる。
///
/// 組み込み`github` serviceは、Sandboxの作成時に`GH_TOKEN`をsentinelで埋める。
/// 登録済みという事実から届いたと推定せず、環境変数を観測する。値は判定にも表示にも
/// 使わず、空かどうかだけを見る。sentinelもtokenも読まない。
pub fn require_token_env_present(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
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
                .try_run(register_command(sandbox)),
        ),
    ))
}

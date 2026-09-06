use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::Observed;
use crate::support::sandbox;

use super::{credential_key, expected_credential_helper};

/// Sandbox内のcredential helperを変更せずに観測する。
///
/// `git config --get`は、keyが無い場合は空出力で答える（終了statusは実装によって
/// ゼロにも非ゼロにもなる）。空出力はどちらの終了statusでも「未設定」として扱う。
/// 値を伴わない失敗だけが、読み取りそのものに失敗した観測不能である。
pub(crate) fn observe_git_credential(
    host: &dyn HostEnvironment,
    sandbox: &str,
) -> Result<Observed> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "config", "--global", "--get", &credential_key()],
    )?;
    let observed = outcome.stdout_text();
    let trimmed = observed.trim();

    if trimmed.is_empty() {
        return Ok(Observed::Missing);
    }
    if !outcome.success() {
        return Ok(Observed::Unobservable {
            evidence: trimmed.to_string(),
        });
    }
    if trimmed == expected_credential_helper() {
        return Ok(Observed::Matching);
    }
    Ok(Observed::Mismatch {
        evidence: trimmed.to_string(),
    })
}

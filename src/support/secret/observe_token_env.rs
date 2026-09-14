use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::Observed;
use crate::support::sandbox;

use super::{TOKEN_ENV_FILE, expected_token_env};

/// Sandbox内のtoken環境変数fileを、変更せずに観測する。
///
/// `cat`はfileが無ければ非ゼロで終わる。中身の無い出力は、どちらの終了statusでも
/// 不在として扱う。空のfileには残すべき内容が無く、上書きしても何も失われない。
/// 中身を伴う失敗だけを、読み取りそのものに失敗した観測不能とする。
pub(crate) fn observe_token_env(
    host: &dyn HostEnvironment,
    sandbox: &str,
    placeholder: &str,
) -> Result<Observed> {
    let outcome = sandbox::exec(host, sandbox, &["cat", TOKEN_ENV_FILE])?;
    let observed = outcome.stdout_text();

    if observed.trim().is_empty() {
        return Ok(Observed::Missing);
    }
    if !outcome.success() {
        return Ok(Observed::Unobservable { evidence: observed });
    }
    if observed == expected_token_env(placeholder) {
        return Ok(Observed::Matching);
    }
    Ok(Observed::Mismatch { evidence: observed })
}

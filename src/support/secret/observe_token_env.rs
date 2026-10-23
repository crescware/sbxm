use crate::diagnostics::Result;

use crate::support::Observed;
use crate::support::sandbox::Inside;

use super::{TOKEN_ENV_FILE, TOKEN_ENV_READ, expected_token_env};

/// fileが存在しないことをprobe自身が確認した終了status。
const MISSING: i32 = 44;

/// Sandbox内のtoken環境変数fileを、変更せずに観測する。
///
/// `cat`だけではfileの不在とpermissionやI/Oの失敗が同じ非ゼロ終了になる。probeが確認した
/// 不在と、正常終了した空fileだけを`Missing`にする。それ以外の非ゼロ終了はstdoutの
/// 有無にかかわらず、読み取りそのものに失敗した観測不能とする。
///
/// 読むcommandは`credential_commands`の2つ目と同じである。
pub(crate) fn observe_token_env(inside: &dyn Inside, placeholder: &str) -> Result<Observed> {
    let outcome = inside.exec(&["sh", "-c", TOKEN_ENV_READ, "sh", TOKEN_ENV_FILE])?;
    let observed = outcome.stdout_text();

    if outcome.status.code() == Some(MISSING) {
        return Ok(Observed::Missing);
    }
    if !outcome.success() {
        return Ok(Observed::Unobservable { evidence: observed });
    }
    if observed.trim().is_empty() {
        return Ok(Observed::Missing);
    }
    if observed == expected_token_env(placeholder) {
        return Ok(Observed::Matching);
    }
    Ok(Observed::Mismatch { evidence: observed })
}

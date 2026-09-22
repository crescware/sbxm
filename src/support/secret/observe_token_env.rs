use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::Observed;
use crate::support::sandbox;

use super::{TOKEN_ENV_FILE, expected_token_env};

/// fileが存在しないことをprobe自身が確認した終了status。
const MISSING: i32 = 44;

/// dangling symlinkは存在する対象として扱い、rootでその行き先を作らない。通常fileも
/// symlinkも無い場合だけ、ほかの読み取り失敗と区別できる終了statusで答える。
const READ: &str = r#"if [ ! -e "$1" ] && [ ! -L "$1" ]; then exit 44; fi; exec cat -- "$1""#;

/// Sandbox内のtoken環境変数fileを、変更せずに観測する。
///
/// `cat`だけではfileの不在とpermissionやI/Oの失敗が同じ非ゼロ終了になる。probeが確認した
/// 不在と、正常終了した空fileだけを`Missing`にする。それ以外の非ゼロ終了はstdoutの
/// 有無にかかわらず、読み取りそのものに失敗した観測不能とする。
pub(crate) fn observe_token_env(
    host: &dyn HostEnvironment,
    sandbox: &str,
    placeholder: &str,
) -> Result<Observed> {
    let outcome = sandbox::exec(host, sandbox, &["sh", "-c", READ, "sh", TOKEN_ENV_FILE])?;
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

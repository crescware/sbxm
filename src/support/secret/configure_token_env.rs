use crate::boundary::host::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;

use crate::support::Observed;
use crate::support::sandbox;

use super::{TOKEN_ENV_FILE, expected_token_env, is_sbxm_token_env, observe_token_env};

/// shellが一切解釈しない形で1行ずつ書き出す。値はargvで渡し、fileへは`printf`が
/// そのまま入れる。
const WRITE: &str = r#"printf '%s\n' "$1" "$2" "$3" > "$4""#;

/// Sandboxの中で`gh`が使う環境変数を、登録済みのplaceholderにする。
///
/// 組み込み`github` serviceは、tokenを1件も保存していなくても`GH_TOKEN`と
/// `GITHUB_TOKEN`をsentinelで埋める。sbxm自身のgitはcredential helperがplaceholderを
/// 持つため影響を受けないが、環境変数を読む`gh`はsentinelを送って401になる。
/// login shellが読むfileでその2つを上書きし、Sandboxの中でも`gh`が通るようにする。
///
/// 書く前に必ず観測する。既に期待どおりならmutationを起こさない。sbxm自身が書いた
/// fileが残っている場合は、現在のplaceholderへ揃える。形の違う中身が既にある場合は、
/// 別の誰かが同じ名前で置いたfileかもしれないため上書きしない。
pub fn configure_token_env(
    host: &dyn HostEnvironment,
    sandbox: &str,
    placeholder: &str,
) -> Result<()> {
    match observe_token_env(host, sandbox, placeholder)? {
        Observed::Matching => Ok(()),
        Observed::Missing => write(host, sandbox, placeholder),
        Observed::Mismatch { evidence } if is_sbxm_token_env(&evidence) => {
            write(host, sandbox, placeholder)
        }
        Observed::Mismatch { .. } => Err(unusable(
            sandbox,
            msg!("cause-token-env-unexpected-content"),
        )),
        Observed::Unobservable { .. } => Err(unusable(sandbox, msg!("cause-token-env-unreadable"))),
    }
}

fn write(host: &dyn HostEnvironment, sandbox: &str, placeholder: &str) -> Result<()> {
    let content = expected_token_env(placeholder);
    let mut lines = content.lines();
    let (Some(marker), Some(first), Some(second)) = (lines.next(), lines.next(), lines.next())
    else {
        // `expected_token_env`は常に3行を返す。
        return Ok(());
    };
    sandbox::exec_as_root(
        host,
        sandbox,
        &[
            "sh",
            "-c",
            WRITE,
            "sh",
            marker,
            first,
            second,
            TOKEN_ENV_FILE,
        ],
    )?
    .require_success()?;
    Ok(())
}

fn unusable(sandbox: &str, reason: Msg) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxTokenEnvUnusable,
            msg!(
                "error-sandbox-token-env-unusable",
                sandbox = sandbox,
                path = TOKEN_ENV_FILE
            ),
        )
        .fact(Fact::reason(reason)),
    )
}

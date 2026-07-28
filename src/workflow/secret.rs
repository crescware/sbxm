//! 案件限定のGitHub credential。
//!
//! tokenの発行と入力は自動化しない。存在確認だけをread-onlyで行い、値は取得も
//! 表示もしない。
//!
//! Sandboxの中へはtokenを渡さない。`sbx secret set-custom`で登録したcustom secretは、
//! Sandboxにplaceholderだけを見せ、登録済みhost宛のrequestに現れたplaceholderをproxyが
//! 本物へ差し替える。service secretを使わないのは、proxyのgithub presetがtokenの形で
//! 扱いを変え、classic personal access tokenを注入しないためである。

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::parse_custom_secrets;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// gitがcredentialを提示する先。
pub const GITHUB_HOST: &str = "github.com";

/// proxyが認証を差し替える対象host。
///
/// 開発中にtokenを提示する先をすべて覆う。登録のないhostへはplaceholderがそのまま
/// 送られ、tokenが正しくても認証されない。gitがgithub.comで通る一方、`gh`が
/// api.github.comで401になっていたのがこれである。
///
/// `sbx secret set-custom --host`はwildcardを受け取る。`*`が1 label、`**`が任意個の
/// labelに一致する。個別のhostを並べるより、subdomainが増えても追随する形を選ぶ。
/// この4件が覆う範囲:
///
/// - `**.github.com`: api（gh、REST、GraphQL）、codeload（tarball、`go get`）、
///   uploads（release asset、添付）、`*.pkg.github.com`（GitHub Packages）、gist
/// - `**.githubusercontent.com`: raw（private repositoryのfile）ほか
///
/// 全hostが1件のcustom secretに載っている必要がある。secretを分けるとplaceholderも
/// 分かれるが、Sandboxの`GH_TOKEN`は1つの値しか持てない。
///
/// ここに並べる文字列は、利用者が実行するcommandへそのまま入り、`sbx secret ls`の
/// `TARGETS`と文字列として突き合わせる。展開した結果ではなく、書いたとおりを比べる。
pub const GITHUB_HOSTS: [&str; 4] = [
    // git clone / fetch / push
    GITHUB_HOST,
    "**.github.com",
    "**.githubusercontent.com",
    // GitHub Container Registry
    "ghcr.io",
];

/// placeholderを受け取るSandbox内の環境変数名。
///
/// 名前を固定するのは、sbxmが案内するcommandと確認する対象を一致させるためである。
pub const GITHUB_TOKEN_ENV: &str = "GH_TOKEN";

/// tokenを登録するcommand。
///
/// `add`の案内と、未登録で停止したときの是正指示で同じ文字列を使う。
///
/// `--host`は`stringArray`であり、繰り返して渡す。区切り文字1つで並べた値は1件のhost
/// 名として読まれる。wildcardはshellに食われるため引用符で囲む。
///
/// 同じenvのcustom secretが既にある場合、`set-custom`はそれを重複として拒否する。
/// 既存のplaceholderを`--placeholder`で明示すると更新として通り、しかもSandboxが
/// 持つ値が変わらないため、作り直さずに済む。
pub fn register_command(sandbox: &str, placeholder: Option<&str>) -> String {
    let hosts = GITHUB_HOSTS
        .iter()
        .map(|host| format!("--host '{host}'"))
        .collect::<Vec<String>>()
        .join(" ");
    let keep = match placeholder {
        Some(placeholder) => format!(" --placeholder {placeholder}"),
        None => String::new(),
    };
    format!(
        "sbx secret set-custom {sandbox} {hosts}{keep} --env {GITHUB_TOKEN_ENV} --value <token>"
    )
}

/// GitHubのcustom secretが登録済みであることを確認する。
///
/// 未登録なら、発行条件と登録commandを示して前提条件不足として停止する。custom secretは
/// Sandboxの作成時に結び付くため、この確認はSandboxを作る前に行う。
pub fn require_github(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    // `--service`で絞ると出力へ値の一部が現れる。一覧のまま読む形で呼ぶ。
    let spec = CommandSpec::capture("sbx", &["secret", "ls", sandbox])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    let customs = parse_custom_secrets(&outcome.stdout_text())?;

    // 1件のsecretが全hostを覆っていることを求める。複数のsecretへ分けて登録すると
    // placeholderが分かれ、Sandboxはそのうち1つしか受け取れない。
    let covered = |custom: &crate::compatibility::CustomSecret| {
        custom.env == GITHUB_TOKEN_ENV
            && GITHUB_HOSTS
                .iter()
                .all(|host| custom.targets.iter().any(|target| target == host))
    };
    if customs.iter().any(covered) {
        return Ok(());
    }

    // 覆われていないhostだけを示す。github.comだけ登録済みの状態から来た場合に、
    // 何が足りないのかがそのまま読める。どのhostも登録はされていて、1件にまとまって
    // いないだけの場合は、まとめる対象として全hostを示す。
    let missing: Vec<&str> = GITHUB_HOSTS
        .iter()
        .filter(|host| {
            !customs.iter().any(|custom| {
                custom.env == GITHUB_TOKEN_ENV
                    && custom.targets.iter().any(|target| target == *host)
            })
        })
        .copied()
        .collect();
    let missing = if missing.is_empty() {
        GITHUB_HOSTS.to_vec()
    } else {
        missing
    };

    // 同じenvのsecretが既にあると、placeholderを指定しない登録は重複として拒否される。
    // 案内どおりに実行しても失敗する状態を作らないため、既存のplaceholderを引き継ぐ形で
    // 示す。同じ値のまま更新されるので、既存Sandboxを作り直さずに済む。
    let existing = customs
        .iter()
        .find(|custom| custom.env == GITHUB_TOKEN_ENV)
        .map(|custom| custom.placeholder.as_str());

    Err(Error::single(
        Diagnostic::new(
            ErrorId::GithubSecretMissing,
            msg!(
                "error-github-secret-missing",
                sandbox = sandbox,
                hosts = missing.join(", ")
            ),
        )
        .remediation(match existing {
            Some(placeholder) => msg!(
                "remediation-github-secret-incomplete",
                command = register_command(sandbox, Some(placeholder))
            ),
            None => msg!(
                "remediation-github-secret-missing",
                command = register_command(sandbox, None)
            ),
        }),
    ))
}

/// Sandboxの中でplaceholderを読むscript。
///
/// 未設定でも失敗させず空文字を返させる。`printenv`のexit statusで分けると、
/// 「設定されていない」と「読めなかった」を区別できない。
pub(super) fn placeholder_probe() -> String {
    format!("printf %s \"${{{GITHUB_TOKEN_ENV}:-}}\"")
}

/// placeholderがSandboxへ届いていることを、中から確かめる。
///
/// custom secretはSandboxの作成時に結び付く。登録済みという事実から届いたと推定せず、
/// 環境変数を観測する。値は判定にも表示にも使わず、空かどうかだけを見る。
pub fn require_placeholder_present(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let outcome = super::sandbox::exec(host, sandbox, &["sh", "-c", &placeholder_probe()])?
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
        .remediation(msg!(
            "remediation-sandbox-secret-not-applied",
            command = format!("sbx rm {sandbox}")
        )),
    ))
}

/// Sandbox内のgitがGitHubへ提示するcredentialのconfig key。
///
/// github.comだけに絞る。ほかのhostへplaceholderを送っても意味がなく、送る相手を
/// 広げる理由がない。
fn credential_key() -> String {
    format!("credential.https://{GITHUB_HOST}.helper")
}

/// Sandbox内のgitに、placeholderをcredentialとして使わせる。
///
/// helperはplaceholderを読むだけで、tokenは持たない。gitはこれをBasic認証として
/// 送り、proxyがgithub.com宛のrequest headerで本物のtokenへ差し替える。usernameは
/// GitHubのgit endpointでは任意の値でよい。
pub fn configure_git_credential(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let helper = format!("!f() {{ echo username=x; echo password=${GITHUB_TOKEN_ENV}; }}; f");
    super::sandbox::exec(
        host,
        sandbox,
        &["git", "config", "--global", &credential_key(), &helper],
    )?
    .require_success()?;
    Ok(())
}

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;

//! 案件限定のGitHub credential。
//!
//! tokenの発行と入力は自動化しない。存在確認だけをread-onlyで行い、値は取得も
//! 表示もしない。
//!
//! Sandboxの中へはtokenを渡さない。`sbx secret set-custom`で登録したcustom secretは、
//! Sandboxにplaceholderだけを見せ、github.com宛のrequest headerでproxyが本物へ
//! 差し替える。service secretを使わないのは、proxyのgithub presetがtokenの形で
//! 扱いを変え、classic personal access tokenを注入しないためである。

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::parse_custom_secrets;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// gitがcredentialを提示する先。
pub const GITHUB_HOST: &str = "github.com";

/// `gh`がGitHub APIを呼ぶ先。
pub const GITHUB_API_HOST: &str = "api.github.com";

/// proxyが認証を差し替える対象host。
///
/// 開発中にtokenを正当に提示する先を並べる。登録のないhostへはplaceholderがそのまま
/// 送られ、tokenが正しくても認証されない。gitがgithub.comだけで通っていた一方で
/// `gh`がapi.github.comで401になっていたのがこれである。
///
/// 全hostが1件のcustom secretに載っている必要がある。secretを分けるとplaceholderも
/// 分かれるが、Sandboxの`GH_TOKEN`は1つの値しか持てない。
///
/// release assetやLFSの実体が載るobjects.githubusercontent.comは入れない。あれは
/// presigned URLで、placeholderを含まないrequestが行くため差し替える対象がない。
pub const GITHUB_HOSTS: [&str; 10] = [
    // git clone / fetch / push
    GITHUB_HOST,
    // gh、REST、GraphQL
    GITHUB_API_HOST,
    // tarball、zipball、`go get`が取りに行く先
    "codeload.github.com",
    // private repositoryのraw file
    "raw.githubusercontent.com",
    // `gh release upload`、issueへの添付
    "uploads.github.com",
    // GitHub Packages
    "npm.pkg.github.com",
    "maven.pkg.github.com",
    "nuget.pkg.github.com",
    "rubygems.pkg.github.com",
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
/// `--host`は繰り返して渡す。区切り文字1つで並べる形は、受け取り側がlistとして解釈
/// する場合にしか通らない。
pub fn register_command(sandbox: &str) -> String {
    let hosts = GITHUB_HOSTS
        .iter()
        .map(|host| format!("--host {host}"))
        .collect::<Vec<String>>()
        .join(" ");
    format!("sbx secret set-custom {sandbox} {hosts} --env {GITHUB_TOKEN_ENV} --value <token>")
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

    Err(Error::single(
        Diagnostic::new(
            ErrorId::GithubSecretMissing,
            msg!(
                "error-github-secret-missing",
                sandbox = sandbox,
                hosts = missing.join(", ")
            ),
        )
        .remediation(msg!(
            "remediation-github-secret-missing",
            command = register_command(sandbox)
        )),
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
mod tests {
    use super::*;
    use crate::command::CommandOutcome;
    use std::cell::RefCell;
    use std::os::unix::process::ExitStatusExt;

    struct FakeSbx {
        listing: String,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl FakeSbx {
        fn listing(output: &str) -> FakeSbx {
            FakeSbx {
                listing: output.to_string(),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl HostEnvironment for FakeSbx {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
            self.calls.borrow_mut().push(spec.args.clone());
            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(0),
                stdout: self.listing.clone().into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

    fn registered() -> String {
        format!(
            "CUSTOM SECRETS\n\
             SCOPE          TARGETS   ENV        PLACEHOLDER      SECRET\n\
             sbxm-example   {}   GH_TOKEN   sbx-cs-example   ghp_example\n",
            GITHUB_HOSTS.join(" ")
        )
    }

    #[test]
    fn a_registered_custom_secret_lets_the_build_continue() {
        let host = FakeSbx::listing(&registered());
        require_github(&host, "sbxm-example").expect("the secret is there");

        let calls = host.calls.borrow();
        assert_eq!(
            calls[0],
            vec![
                "secret".to_string(),
                "ls".to_string(),
                "sbxm-example".to_string()
            ],
            "the check is read-only and never asks for the value"
        );
    }

    #[test]
    fn a_service_secret_is_not_accepted_in_place_of_a_custom_one() {
        // service secretはproxyのgithub presetを通り、classic tokenを注入しない。
        // 登録されていても、Sandboxがrepositoryへ届くとは限らない。
        let host = FakeSbx::listing(
            "SCOPE          TYPE      NAME     SECRET\nsbxm-example   service   github   (stored)\n",
        );
        let error = require_github(&host, "sbxm-example")
            .expect_err("a service secret does not carry every token type");

        assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    }

    #[test]
    fn covering_only_the_git_host_leaves_gh_unauthenticated_and_is_refused() {
        // gitはgithub.comへ、ghはapi.github.comへ話す。この登録ではgit push/fetchだけが
        // 通り、`gh`はplaceholderをそのまま送って401になる。
        let host = FakeSbx::listing(
            "CUSTOM SECRETS\n\
             SCOPE          TARGETS      ENV        PLACEHOLDER      SECRET\n\
             sbxm-example   github.com   GH_TOKEN   sbx-cs-example   ghp_example\n",
        );
        let error = require_github(&host, "sbxm-example")
            .expect_err("the proxy substitutes only for the hosts it was told about");

        assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
        let missing = error.diagnostics()[0]
            .description
            .args
            .iter()
            .find(|(name, _)| *name == "hosts")
            .map(|(_, value)| value.clone())
            .expect("the message names what is not covered");
        let missing: Vec<&str> = missing.split(", ").collect();
        assert!(
            missing.contains(&GITHUB_API_HOST),
            "the host gh talks to is named: {missing:?}"
        );
        assert!(
            !missing.contains(&GITHUB_HOST),
            "a host that is already covered is not reported as missing: {missing:?}"
        );
    }

    #[test]
    fn the_hosts_have_to_share_one_secret_so_that_they_share_one_placeholder() {
        // 両hostが登録されていても、別々のsecretならplaceholderが2つになる。Sandboxの
        // GH_TOKENは1つしか持てないので、片方は必ず素通しになる。
        let host = FakeSbx::listing(&format!(
            "CUSTOM SECRETS\n\
             SCOPE          TARGETS   ENV        PLACEHOLDER      SECRET\n\
             sbxm-example   {}   GH_TOKEN   sbx-cs-one       ghp_example\n\
             sbxm-example   {}   GH_TOKEN   sbx-cs-two       ghp_example\n",
            GITHUB_HOST,
            GITHUB_HOSTS[1..].join(" ")
        ));
        let error = require_github(&host, "sbxm-example")
            .expect_err("two placeholders cannot both reach one environment variable");

        assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    }

    #[test]
    fn a_custom_secret_for_another_host_does_not_count() {
        let host = FakeSbx::listing(
            "CUSTOM SECRETS\n\
             SCOPE          TARGETS      ENV        PLACEHOLDER      SECRET\n\
             sbxm-example   gitlab.com   GH_TOKEN   sbx-cs-example   ghp_example\n",
        );
        let error = require_github(&host, "sbxm-example")
            .expect_err("the proxy only substitutes for the hosts it was told about");

        assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    }

    #[test]
    fn a_missing_secret_stops_with_the_command_that_registers_it() {
        let host = FakeSbx::listing("No secrets found for scope \"sbxm-example\".\n");
        let error = require_github(&host, "sbxm-example")
            .expect_err("a build without repository access cannot continue");

        assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
        let remediation = error.diagnostics()[0]
            .remediation
            .as_ref()
            .expect("the user is told how to register it");
        assert_eq!(remediation.id, "remediation-github-secret-missing");
        assert!(
            remediation
                .args
                .iter()
                .any(|(_, value)| value == &register_command("sbxm-example"))
        );
    }

    #[test]
    fn a_sandbox_that_carries_the_placeholder_passes_the_check() {
        let host = FakeSbx::listing("sbx-cs-example");
        require_placeholder_present(&host, "sbxm-example").expect("the placeholder is there");
    }

    #[test]
    fn a_sandbox_without_the_placeholder_is_refused_instead_of_assumed_ready() {
        // 登録済みという事実からSandboxへ届いたと推定しない。作成より後に登録した場合、
        // secretは一覧に並んでいてもSandboxの中には現れない。
        let host = FakeSbx::listing("");
        let error = require_placeholder_present(&host, "sbxm-example")
            .expect_err("git inside cannot authenticate without the placeholder");

        assert_eq!(error.first_id(), Some(ErrorId::SandboxSecretNotApplied));
        let remediation = error.diagnostics()[0]
            .remediation
            .as_ref()
            .expect("the user is told how to get out of it");
        assert!(
            remediation
                .args
                .iter()
                .any(|(_, value)| value == "sbx rm sbxm-example")
        );
    }

    #[test]
    fn the_credential_helper_reads_the_placeholder_and_holds_no_token() {
        let host = FakeSbx::listing("");
        configure_git_credential(&host, "sbxm-example").expect("the helper is configured");

        let call = host.calls.borrow()[0].join(" ");
        assert!(call.contains("credential.https://github.com.helper"));
        // helperはSandboxの環境変数を読むだけで、値そのものは持たない。
        assert!(call.contains("password=$GH_TOKEN"));
    }

    #[test]
    fn the_command_sbxm_prints_registers_exactly_what_sbxm_checks_for() {
        // 案内と検査がずれると、案内どおりに実行しても止まり続ける状態になる。
        let command = register_command("sbxm-example");
        for host in GITHUB_HOSTS {
            assert!(
                command.contains(&format!("--host {host}")),
                "{host} is checked for, so it has to be registered: {command}"
            );
        }
        assert_eq!(
            command.matches("--host ").count(),
            GITHUB_HOSTS.len(),
            "no host is registered that is never checked for: {command}"
        );
        assert_eq!(
            command.matches("--env ").count(),
            1,
            "one secret carries every host, so one placeholder reaches GH_TOKEN: {command}"
        );
    }

    #[test]
    fn the_value_of_a_secret_is_never_requested() {
        let host = FakeSbx::listing(&registered());
        require_github(&host, "sbxm-example").expect("the secret is there");

        for args in host.calls.borrow().iter() {
            assert!(
                !args.iter().any(|arg| arg == "get" || arg == "reveal"),
                "sbxm only asks whether the secret exists: {args:?}"
            );
        }
    }
}

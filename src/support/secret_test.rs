use super::*;
use crate::command::CommandOutcome;
use std::cell::RefCell;

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
        Ok(crate::testing::command::outcome(spec, 0, &self.listing))
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
fn the_listing_a_real_registration_produces_is_accepted() {
    // 実機で`sbx secret set-custom`を実行したあとの`sbx secret ls`の形。scope名と
    // secretは記録から伏せてある。`TARGETS`はcommaと空白1つで区切られ、wildcardは
    // 展開されずに並ぶため、sbxmが書いた文字列とそのまま突き合わせられる。
    let host = FakeSbx::listing(
        "CUSTOM SECRETS\n\
             SCOPE          TARGETS                                                        ENV        PLACEHOLDER               SECRET\n\
             sbxm-example   github.com, **.github.com, **.githubusercontent.com, ghcr.io   GH_TOKEN   sbx-cs-Y1k0SfTWbkN6HzCO   ghp_redacted\n",
    );
    require_github(&host, "sbxm-example").expect("the registration this build asks for passes");
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
    // 通り、`gh`はplaceholderをそのまま送って401になる。実機で起きたのがこの状態。
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
        missing.contains(&"**.github.com"),
        "the pattern that covers api.github.com is named: {missing:?}"
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
            .any(|(_, value)| value == &register_command("sbxm-example", None))
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
fn an_incomplete_secret_is_told_to_keep_the_placeholder_the_sandbox_already_holds() {
    // placeholderを指定しない登録は、同じenvが既にあると重複として拒否される。
    // 既存の値を引き継ぐ形で示さないと、案内どおりに実行しても必ず失敗する。
    let host = FakeSbx::listing(
        "CUSTOM SECRETS\n\
             SCOPE          TARGETS      ENV        PLACEHOLDER              SECRET\n\
             sbxm-example   github.com   GH_TOKEN   sbx-cs-Y1k0SfTWbkN6HzCO  ghp_example\n",
    );
    let error = require_github(&host, "sbxm-example").expect_err("the coverage is incomplete");

    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .expect("the user is told how to get out of it");
    assert_eq!(remediation.id, "remediation-github-secret-incomplete");
    let command = remediation
        .args
        .iter()
        .find(|(name, _)| *name == "command")
        .map(|(_, value)| value.clone())
        .expect("the remediation carries the command to run");
    assert!(
        command.contains("--placeholder sbx-cs-Y1k0SfTWbkN6HzCO"),
        "the existing placeholder is carried over: {command}"
    );
    // 同じplaceholderのまま更新されるため、Sandboxが持つ値は変わらない。
    assert!(
        !command.contains("sbx rm"),
        "keeping the placeholder means the sandbox does not have to be rebuilt: {command}"
    );
}

#[test]
fn a_sandbox_with_no_secret_at_all_is_told_to_register_one_without_a_placeholder() {
    let host = FakeSbx::listing("No secrets found for scope \"sbxm-example\".\n");
    let error = require_github(&host, "sbxm-example").expect_err("nothing is registered");

    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .expect("present");
    assert_eq!(remediation.id, "remediation-github-secret-missing");
    assert!(
        remediation
            .args
            .iter()
            .all(|(_, value)| !value.contains("--placeholder")),
        "there is no placeholder to keep, so none is invented"
    );
}

#[test]
fn the_command_sbxm_prints_registers_exactly_what_sbxm_checks_for() {
    // 案内と検査がずれると、案内どおりに実行しても止まり続ける状態になる。
    let command = register_command("sbxm-example", None);
    for host in GITHUB_HOSTS {
        assert!(
            command.contains(&format!("--host '{host}'")),
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

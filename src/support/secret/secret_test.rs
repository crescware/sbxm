use crate::boundary::host::{CommandSpec, HostEnvironment};
use crate::diagnostics::{ErrorId, Result};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::boundary::host::CommandOutcome;
use crate::project::ProjectId;
use std::cell::RefCell;

struct FakeSbx {
    /// `secret ls`へ順に返す出力。末尾から取り出し、最後の1件は繰り返す。
    listings: RefCell<Vec<String>>,
    calls: RefCell<Vec<Vec<String>>>,
    /// Sandbox内のcommandが返す終了statusと出力。argvの一部で選ぶ。
    inside: RefCell<Vec<(&'static str, i32, &'static str, &'static str)>>,
}

impl FakeSbx {
    fn listing(output: &str) -> FakeSbx {
        FakeSbx::listings(&[output])
    }

    /// mutationの前後で観測が変わるhost。
    fn listings(outputs: &[&str]) -> FakeSbx {
        FakeSbx {
            listings: RefCell::new(
                outputs
                    .iter()
                    .rev()
                    .map(|value| (*value).to_string())
                    .collect(),
            ),
            calls: RefCell::new(Vec::new()),
            inside: RefCell::new(Vec::new()),
        }
    }

    /// `needle`を含むSandbox内のcommandへ、この終了statusと出力で答える。
    fn inside(
        self,
        needle: &'static str,
        code: i32,
        stdout: &'static str,
        stderr: &'static str,
    ) -> FakeSbx {
        self.inside
            .borrow_mut()
            .push((needle, code, stdout, stderr));
        self
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.args.clone());
        let joined = spec.args.join(" ");
        if let Some((_, code, stdout, stderr)) = self
            .inside
            .borrow()
            .iter()
            .find(|(needle, ..)| joined.contains(needle))
        {
            let mut outcome = crate::testing::command::outcome(spec, *code, stdout);
            outcome.stderr = stderr.as_bytes().to_vec();
            return Ok(outcome);
        }
        let reads_listing = spec.args.first().is_some_and(|arg| arg == "secret")
            && spec.args.get(1).is_some_and(|arg| arg == "ls");
        if !reads_listing {
            return Ok(crate::testing::command::outcome(spec, 0, ""));
        }
        let mut listings = self.listings.borrow_mut();
        let output = listings.last().cloned().unwrap_or_default();
        // 一覧を読み直す工程だけが次の観測へ進む。最後の1件は繰り返す。
        if listings.len() > 1 {
            listings.pop();
        }
        Ok(crate::testing::command::outcome(spec, 0, &output))
    }
}

/// 1件のcustom secretが全hostを覆う一覧。
fn scoped(scope: &str, placeholder: &str) -> String {
    format!(
        "CUSTOM SECRETS\n\
             SCOPE          TARGETS   ENV        PLACEHOLDER      SECRET\n\
             {scope}   {}   GH_TOKEN   {placeholder}   ghp_example\n",
        GITHUB_HOSTS.join(" ")
    )
}

fn registered() -> String {
    scoped("sbxm-example", "sbx-cs-example")
}

/// 1件も登録がないscopeの一覧。
fn none() -> String {
    "No secrets found for scope \"sbxm-example\".\n".to_string()
}

fn project() -> Checked<ProjectId> {
    ProjectId::parse("example-org/example-repo").required()
}

#[test]
fn a_registered_custom_secret_lets_the_build_continue_and_names_its_placeholder() -> Checked {
    let host = FakeSbx::listing(&registered());
    let registration =
        require_github(&host, "sbxm-example").required_because("the secret is there")?;
    assert_eq!(registration.scope(), "sbxm-example");
    assert_eq!(registration.placeholder(), "sbx-cs-example");

    let calls = host.calls.borrow();
    assert_eq!(
        calls[0],
        vec!["secret".to_string(), "ls".to_string()],
        "the check is read-only, never asks for the value, and never narrows the scope"
    );
    Ok(())
}

#[test]
fn a_service_secret_is_not_accepted_in_place_of_a_custom_one() -> Checked {
    // これはsbxmの根幹の判断であり、実機の測定に基づく（PR #10 / commit 4cb8907）。
    // 同じclassic personal access tokenを`github` service secretとして登録し、
    // Sandboxの外と中から同じrequestを投げた結果:
    //
    // | request                          | 外  | 中  |
    // |----------------------------------|----:|----:|
    // | api.github.com/user（Bearer）    | 200 | 401 |
    // | github.comのgit endpoint（Basic）| 200 | 401 |
    //
    // proxyのgithub presetはtokenの形で扱いを変え、classic tokenを注入しない。
    // service secretを受け付けると、classic tokenの利用者は登録できたのに認証が
    // 通らない状態に落ちる。custom secretはtokenの形を問わない。
    let host = FakeSbx::listing(
        "SCOPE          TYPE      NAME     SECRET\nsbxm-example   service   github   (stored)\n",
    );
    let error = require_github(&host, "sbxm-example")
        .refused_because("a service secret does not carry every token type")?;

    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    Ok(())
}

#[test]
fn the_command_sbxm_prints_registers_a_custom_secret_so_that_any_token_shape_works() {
    // 案内をservice secretへ変えると、classic tokenの利用者が認証できなくなる。
    // 上のtestと対で、案内と検査の両側からこの判断を固定する。
    let command = register_command("sbxm-example", None);
    assert!(
        command.starts_with("sbx secret set-custom "),
        "the token is registered as a custom secret, not as a service secret: {command}"
    );
    assert!(
        !command.contains("secret set github") && !command.contains("--service"),
        "nothing asks the proxy to interpret the token by its shape: {command}"
    );
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
        "one secret carries every host, so one placeholder covers them all: {command}"
    );
}

#[test]
fn the_credential_the_sandbox_presents_is_the_placeholder_and_never_the_token() -> Checked {
    // helperが持つのはplaceholderだけである。tokenの形にも長さにも依存しないため、
    // classic tokenでもfine-grained tokenでも、Sandboxの中の手順は同じになる。
    for placeholder in ["sbx-cs-example", "sbx-cs-J0uA6pOfxmdzMF1W"] {
        let host = FakeSbx::listing("");
        configure_git_credential(&host, "sbxm-example", placeholder)
            .required_because("the helper is configured")?;
        let written = host
            .calls
            .borrow()
            .iter()
            .map(|call| call.join(" "))
            .find(|call| call.contains("credential.https://github.com.helper !f"))
            .required_because("the helper is written")?;
        assert!(
            written.contains(&format!("password={placeholder};")),
            "the placeholder is what git presents: {written}"
        );
        assert!(
            !written.contains("ghp_") && !written.contains("github_pat_"),
            "no token material reaches the sandbox: {written}"
        );
    }
    Ok(())
}

#[test]
fn covering_only_the_git_host_leaves_gh_unauthenticated_and_is_refused() -> Checked {
    // gitはgithub.comへ、ghはapi.github.comへ話す。この登録ではgit push/fetchだけが
    // 通り、`gh`はplaceholderをそのまま送って401になる。実機で起きたのがこの状態。
    let host = FakeSbx::listing(
        "CUSTOM SECRETS\n\
             SCOPE          TARGETS      ENV        PLACEHOLDER      SECRET\n\
             sbxm-example   github.com   GH_TOKEN   sbx-cs-example   ghp_example\n",
    );
    let error = require_github(&host, "sbxm-example")
        .refused_because("the proxy substitutes only for the hosts it was told about")?;

    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    let missing = error.diagnostics()[0]
        .description
        .args
        .iter()
        .find(|(name, _)| *name == "hosts")
        .map(|(_, value)| value.clone())
        .required_because("the message names what is not covered")?;
    let missing: Vec<&str> = missing.split(", ").collect();
    assert!(
        missing.contains(&"**.github.com"),
        "the pattern that covers api.github.com is named: {missing:?}"
    );
    assert!(
        !missing.contains(&GITHUB_HOST),
        "a host that is already covered is not reported as missing: {missing:?}"
    );
    Ok(())
}

#[test]
fn a_registration_bound_to_another_sandbox_does_not_count() -> Checked {
    // 別のSandboxのscopeへ結び付いた登録は、このSandboxのrequestでは差し替えられない。
    let host = FakeSbx::listing(&scoped("sbxm-another", "sbx-cs-elsewhere"));
    let error = require_github(&host, "sbxm-example")
        .refused_because("another sandbox's registration never reaches this one")?;
    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    Ok(())
}

#[test]
fn a_global_registration_counts_for_this_sandbox() -> Checked {
    let host = FakeSbx::listing(&scoped("(global)", "sbx-cs-global"));
    let registration = require_github(&host, "sbxm-example")
        .required_because("a global registration reaches every sandbox")?;
    assert_eq!(registration.scope(), "(global)");
    assert_eq!(registration.placeholder(), "sbx-cs-global");
    Ok(())
}

#[test]
fn replacement_keeps_the_selected_registration_scope_and_placeholder() -> Checked {
    for (scope, expected_prefix) in [
        (
            "sbxm-example",
            "sbx secret set-custom sbxm-example --host 'github.com'",
        ),
        ("(global)", "sbx secret set-custom --host 'github.com'"),
    ] {
        let host = FakeSbx::listing(&scoped(scope, "sbx-cs-existing"));
        let command = replace_github_command(&host, "sbxm-example")
            .required_because("the selected registration can be replaced")?;

        assert!(command.starts_with(expected_prefix), "{scope}: {command}");
        assert!(
            command.contains("--placeholder sbx-cs-existing"),
            "{scope}: {command}"
        );
        assert!(command.ends_with("--value <token>"), "{scope}: {command}");
        assert!(
            !command.contains("ghp_example"),
            "the SECRET column is never copied: {command}"
        );
    }

    let host = FakeSbx::listing(&format!(
        "CUSTOM SECRETS\n\
         SCOPE          TARGETS   ENV        PLACEHOLDER      SECRET\n\
         sbxm-example   {}   GH_TOKEN   sbx-cs-one   ghp_one\n\
         sbxm-example   {}   OTHER      sbx-cs-two   ghp_two\n",
        GITHUB_HOSTS.join(" "),
        GITHUB_HOSTS.join(" ")
    ));
    let error = replace_github_command(&host, "sbxm-example")
        .refused_because("two registrations cannot be rotated as if they were one")?;
    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    Ok(())
}

#[test]
fn a_sandbox_scoped_registration_wins_over_a_global_one() -> Checked {
    // 両方ある場合、Sandboxへ結び付いた側が使われる。案件ごとのtokenを、ほかの案件と
    // 共有するglobalの登録で上書きしない。
    let host = FakeSbx::listing(&format!(
        "CUSTOM SECRETS\n\
             SCOPE          TARGETS   ENV        PLACEHOLDER      SECRET\n\
             (global)   {}   GH_TOKEN   sbx-cs-global   ghp_example\n\
             sbxm-example   {}   GH_TOKEN   sbx-cs-scoped   ghp_example\n",
        GITHUB_HOSTS.join(" "),
        GITHUB_HOSTS.join(" ")
    ));
    let registration =
        require_github(&host, "sbxm-example").required_because("the scoped one is chosen")?;
    assert_eq!(registration.scope(), "sbxm-example");
    assert_eq!(registration.placeholder(), "sbx-cs-scoped");
    Ok(())
}

#[test]
fn several_registrations_for_one_sandbox_are_refused_instead_of_picked_from() -> Checked {
    // どちらのplaceholderをgitへ持たせるかを選ぶと、食い違ったまま静かに失敗する。
    let host = FakeSbx::listing(&format!(
        "CUSTOM SECRETS\n\
             SCOPE          TARGETS   ENV        PLACEHOLDER      SECRET\n\
             sbxm-example   {}   GH_TOKEN   sbx-cs-one   ghp_example\n\
             sbxm-example   {}   OTHER      sbx-cs-two   ghp_example\n",
        GITHUB_HOSTS.join(" "),
        GITHUB_HOSTS.join(" ")
    ));
    let error = require_github(&host, "sbxm-example")
        .refused_because("two placeholders cannot both be presented")?;
    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    assert!(format!("{error:?}").contains("cause-github-secret-ambiguous"));
    Ok(())
}

#[test]
fn a_missing_secret_stops_with_the_command_that_registers_it() -> Checked {
    let host = FakeSbx::listing(&none());
    let error = require_github(&host, "sbxm-example")
        .refused_because("a build without repository access cannot continue")?;

    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("the user is told how to register it")?;
    assert_eq!(
        remediation.explanation[0].id,
        "remediation-github-secret-missing"
    );
    assert!(
        remediation
            .commands
            .iter()
            .any(|command| command.as_str() == register_command("sbxm-example", None))
    );
    Ok(())
}

#[test]
fn an_incomplete_secret_is_told_to_keep_the_placeholder_the_sandbox_already_holds() -> Checked {
    // placeholderを指定しない登録は、同じenvが既にあると重複として拒否される。
    // 既存の値を引き継ぐ形で示さないと、案内どおりに実行しても必ず失敗する。
    let host = FakeSbx::listing(
        "CUSTOM SECRETS\n\
             SCOPE          TARGETS      ENV        PLACEHOLDER              SECRET\n\
             sbxm-example   github.com   GH_TOKEN   sbx-cs-Y1k0SfTWbkN6HzCO  ghp_example\n",
    );
    let error =
        require_github(&host, "sbxm-example").refused_because("the coverage is incomplete")?;

    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("the user is told how to get out of it")?;
    assert_eq!(
        remediation.explanation[0].id,
        "remediation-github-secret-incomplete"
    );
    let command = remediation
        .commands
        .first()
        .map(|command| command.as_str().to_string())
        .required_because("the remediation carries the command to run")?;
    assert!(
        command.contains("--placeholder sbx-cs-Y1k0SfTWbkN6HzCO"),
        "the existing placeholder is carried over: {command}"
    );
    assert!(
        !command.contains("sbx rm"),
        "keeping the placeholder means the sandbox does not have to be rebuilt: {command}"
    );
    Ok(())
}

#[test]
fn the_registration_is_removed_by_its_placeholder_and_verified_gone() -> Checked {
    let host = FakeSbx::listings(&[&registered(), &none()]);
    let removed =
        forget_github(&host, "sbxm-example").required_because("the registration is removed")?;
    assert_eq!(removed, vec!["sbx-cs-example".to_string()]);

    let calls = host.calls.borrow();
    assert_eq!(
        format!("sbx {}", calls[1].join(" ")),
        forget_command("sbxm-example", "sbx-cs-example"),
        "sbxm runs exactly the command it names when the removal has to be repeated by hand"
    );
    assert_eq!(
        calls.len(),
        3,
        "the listing is read again, so absence is observed instead of assumed: {calls:?}"
    );
    Ok(())
}

#[test]
fn a_registration_of_another_scope_is_not_removed_with_this_sandbox() -> Checked {
    // global scopeのsecretはほかのSandboxも使う。1案件の後片付けで消す対象ではない。
    let host = FakeSbx::listing(&scoped("(global)", "sbx-cs-elsewhere"));
    let removed = forget_github(&host, "sbxm-example")
        .required_because("nothing of this project is there")?;
    assert!(removed.is_empty());
    assert!(
        !host
            .calls
            .borrow()
            .iter()
            .any(|args| args.contains(&"rm".to_string())),
        "nothing is removed: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn a_secret_the_user_registered_for_something_else_is_left_alone() -> Checked {
    // 同じscopeへ別のsecretを登録していることがある。sbxmは自分が案内した登録だけを扱う。
    let host = FakeSbx::listing(
        "CUSTOM SECRETS\n\
             SCOPE          TARGETS            ENV                 PLACEHOLDER      SECRET\n\
             sbxm-example   api.example.com    ANTHROPIC_API_KEY   sbx-cs-other     sk-example\n",
    );
    let removed = forget_github(&host, "sbxm-example")
        .required_because("nothing sbxm registered is there")?;
    assert!(removed.is_empty());
    Ok(())
}

#[test]
fn nothing_is_removed_when_no_token_was_ever_registered() -> Checked {
    let host = FakeSbx::listing(&none());
    assert!(
        forget_github(&host, "sbxm-example")
            .required_because("an unregistered scope is not an error")?
            .is_empty()
    );
    assert_eq!(host.calls.borrow().len(), 1, "only the listing is read");
    Ok(())
}

#[test]
fn a_registration_that_survives_the_removal_is_reported_with_the_command_to_run() -> Checked {
    // 一覧に残り続ける。消えたことを確かめるまで完了としない。
    let host = FakeSbx::listing(&registered());
    let error =
        forget_github(&host, "sbxm-example").refused_because("the registration is still listed")?;

    assert_eq!(error.first_id(), Some(ErrorId::SecretStillRegistered));
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("the user is told how to remove it")?;
    assert!(
        remediation
            .commands
            .iter()
            .any(|command| command.as_str() == forget_command("sbxm-example", "sbx-cs-example"))
    );
    Ok(())
}

#[test]
fn the_value_of_a_secret_is_never_named_or_requested() -> Checked {
    let host = FakeSbx::listings(&[&registered(), &none()]);
    require_github(&host, "sbxm-example").required_because("the secret is there")?;
    forget_github(&host, "sbxm-example").required_because("the registration is removed")?;

    for args in host.calls.borrow().iter() {
        assert!(
            !args
                .iter()
                .any(|arg| arg == "--value" || arg == "--token" || arg == "get" || arg == "reveal"),
            "sbxm only names the placeholder: {args:?}"
        );
    }
    Ok(())
}

#[test]
fn the_credential_helper_is_written_when_nothing_is_set_yet() -> Checked {
    let host = FakeSbx::listing("");
    configure_git_credential(&host, "sbxm-example", "sbx-cs-example")
        .required_because("the helper is configured")?;

    let calls = host.calls.borrow();
    assert!(
        calls[0]
            .join(" ")
            .contains("credential.https://github.com.helper"),
        "the current value is read before anything is written: {calls:?}"
    );
    assert_eq!(calls.len(), 2, "read once, write once: {calls:?}");
    Ok(())
}

#[test]
fn configure_leaves_an_existing_matching_credential_helper_untouched() -> Checked {
    let host = FakeSbx::listing("").inside(
        "--get credential.https://github.com.helper",
        0,
        "!f() { echo username=x; echo password=sbx-cs-example; }; f",
        "",
    );
    configure_git_credential(&host, "sbxm-example", "sbx-cs-example")
        .required_because("the helper already matches")?;
    assert_eq!(
        host.calls.borrow().len(),
        1,
        "the value is read once and never written: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn a_helper_sbxm_wrote_earlier_is_brought_up_to_the_current_placeholder() -> Checked {
    // tokenを登録し直すとplaceholderが変わる。旧版は環境変数を読む形で書いていた。
    // どちらも次のfetchが認証できなくなるため、観測したその場で書き換える。
    for stale in [
        "!f() { echo username=x; echo password=$GH_TOKEN; }; f",
        "!f() { echo username=x; echo password=sbx-cs-old; }; f",
    ] {
        let host =
            FakeSbx::listing("").inside("--get credential.https://github.com.helper", 0, stale, "");
        configure_git_credential(&host, "sbxm-example", "sbx-cs-new")
            .required_because("a value sbxm wrote is updated, not refused")?;
        let written = host
            .calls
            .borrow()
            .iter()
            .map(|call| call.join(" "))
            .find(|call| call.contains("password=sbx-cs-new"))
            .required_because("the current placeholder is written");
        assert!(written.is_ok(), "{stale} is replaced");
    }
    Ok(())
}

#[test]
fn configure_refuses_a_different_credential_helper() -> Checked {
    let host =
        FakeSbx::listing("").inside("--get credential.https://github.com.helper", 0, "store", "");
    let error = configure_git_credential(&host, "sbxm-example", "sbx-cs-example")
        .refused_because("a helper set to something else may belong to another user's sandbox")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::SandboxCredentialHelperUnusable)
    );
    assert_eq!(
        host.calls.borrow().len(),
        1,
        "the mismatch is refused before any write is attempted: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn configure_refuses_a_credential_helper_it_cannot_observe() -> Checked {
    let host = FakeSbx::listing("").inside(
        "--get credential.https://github.com.helper",
        1,
        "fatal: bad config",
        "",
    );
    let error = configure_git_credential(&host, "sbxm-example", "sbx-cs-example")
        .refused_because("a failure that answered with text is not the same as an unset key")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::SandboxCredentialHelperUnusable)
    );
    assert_eq!(
        host.calls.borrow().len(),
        1,
        "nothing is written while the existing value cannot be read: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn github_accepting_the_credential_lets_the_build_continue() -> Checked {
    let host = FakeSbx::listing("");
    let registration = GithubRegistration::new("sbxm-example", "sbx-cs-example");
    require_github_accepts(&host, "sbxm-example", &project()?, &registration)
        .required_because("GitHub answered the probe")?;
    let probe = host.calls.borrow()[0].join(" ");
    assert!(
        probe.contains("ls-remote")
            && probe.contains("https://github.com/example-org/example-repo.git"),
        "the probe takes the same path as the fetch that follows: {probe}"
    );
    assert!(
        probe.contains("GIT_TERMINAL_PROMPT=0"),
        "a probe without a terminal must not wait for a username: {probe}"
    );
    Ok(())
}

#[test]
fn a_credential_github_rejects_is_updated_with_its_existing_placeholder() -> Checked {
    // tokenが無効でも、proxyが差し替えずplaceholderがそのまま届いても、GitHubは同じ
    // 文で拒む。どちらも登録し直すところから始まる。
    for stderr in [
        "remote: Invalid username or token. Password authentication is not supported for Git operations.\nfatal: Authentication failed for 'https://github.com/example-org/example-repo.git/'\n",
        "fatal: could not read Username for 'https://github.com': No such device or address\n",
    ] {
        let host = FakeSbx::listings(&[
            &scoped("sbxm-example", "sbx-cs-rejected"),
            &scoped("sbxm-example", "sbx-cs-replaced"),
        ])
        .inside("ls-remote", 128, "", stderr);
        let registration = require_github(&host, "sbxm-example")
            .required_because("the registered placeholder is used for authentication")?;
        let error = require_github_accepts(&host, "sbxm-example", &project()?, &registration)
            .refused_because("a rejected credential stops the build before the fetch")?;
        assert_eq!(error.first_id(), Some(ErrorId::GithubCredentialRejected));
        let diagnostic = &error.diagnostics()[0];
        let remediation = diagnostic
            .remediation
            .as_ref()
            .required_because("the user is told how to update the existing secret")?;
        assert_eq!(
            remediation.explanation[0].id,
            "remediation-github-credential-rejected"
        );
        assert_eq!(
            remediation
                .commands
                .iter()
                .map(crate::design::text::CommandLine::as_str)
                .collect::<Vec<_>>(),
            [
                "sbx secret set-custom sbxm-example --host 'github.com' --host '**.github.com' --host '**.githubusercontent.com' --host 'ghcr.io' --placeholder sbx-cs-rejected --env GH_TOKEN --value <token>"
            ],
            "the update preserves the placeholder used by git and needs no deletion or rebuild"
        );
        assert_eq!(
            diagnostic
                .external
                .as_ref()
                .required_because("the failed probe is preserved")?
                .stderr,
            stderr.as_bytes(),
            "the original GitHub error remains available"
        );
        assert_eq!(
            host.calls.borrow().len(),
            2,
            "the registration is read once; the rejection does not reread or mutate secrets"
        );
    }
    Ok(())
}

#[test]
fn a_rejected_global_registration_is_updated_in_global_scope() -> Checked {
    let host = FakeSbx::listing(&scoped("(global)", "sbx-cs-global")).inside(
        "ls-remote",
        128,
        "",
        "remote: Invalid username or token.\nfatal: Authentication failed\n",
    );
    let registration = require_github(&host, "sbxm-example")
        .required_because("the global registration is selected")?;
    let error = require_github_accepts(&host, "sbxm-example", &project()?, &registration)
        .refused_because("the global credential was rejected")?;
    let command = error.diagnostics()[0]
        .remediation
        .as_ref()
        .and_then(|remediation| remediation.commands.first())
        .map(crate::design::text::CommandLine::as_str)
        .required_because("the update command is shown")?;
    assert_eq!(
        command,
        "sbx secret set-custom --host 'github.com' --host '**.github.com' --host '**.githubusercontent.com' --host 'ghcr.io' --placeholder sbx-cs-global --env GH_TOKEN --value <token>",
        "global is the default scope, so no sandbox name is supplied"
    );
    Ok(())
}

#[test]
fn a_failure_that_is_not_a_rejection_is_reported_as_the_command_that_failed() -> Checked {
    // 到達できない、repositoryが無いといった失敗は、tokenを登録し直しても直らない。
    let host = FakeSbx::listing("").inside(
        "ls-remote",
        128,
        "",
        "fatal: unable to access 'https://github.com/example-org/example-repo.git/': Could not resolve host: github.com\n",
    );
    let registration = GithubRegistration::new("sbxm-example", "sbx-cs-example");
    let error = require_github_accepts(&host, "sbxm-example", &project()?, &registration)
        .refused_because("the probe failed for another reason")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}

/// `gh`が読む環境変数fileの、期待する中身。
fn token_env(placeholder: &str) -> String {
    format!(
        "# Written by sbxm. The placeholder is substituted by the Docker Sandboxes proxy.\n\
         export GH_TOKEN={placeholder}\n\
         export GITHUB_TOKEN={placeholder}\n"
    )
}

#[test]
fn the_variables_gh_reads_are_set_to_the_placeholder() -> Checked {
    // 組み込み`github` serviceは、tokenを1件も保存していなくても`GH_TOKEN`と
    // `GITHUB_TOKEN`をsentinelで埋める。実機では`gho_sbxproxymanaged…`が入り、
    // `gh`はそれを送って401になる。login shellが読むfileで両方を上書きする。
    let host = FakeSbx::listing("");
    configure_token_env(&host, "sbxm-example", "sbx-cs-example")
        .required_because("the file is written")?;

    let written = host
        .calls
        .borrow()
        .iter()
        .map(|call| call.join(" "))
        .find(|call| call.contains("printf"))
        .required_because("the file is written")?;
    assert!(
        written.contains("--user root"),
        "the file lives under /etc, so it is written as root: {written}"
    );
    for variable in ["GH_TOKEN", "GITHUB_TOKEN"] {
        assert!(
            written.contains(&format!("export {variable}=sbx-cs-example")),
            "{variable} is overridden, so nothing sends the sentinel: {written}"
        );
    }
    assert!(
        !written.contains("ghp_") && !written.contains("github_pat_"),
        "no token material reaches the sandbox: {written}"
    );
    Ok(())
}

#[test]
fn a_matching_token_env_file_is_left_untouched() -> Checked {
    let host = FakeSbx::listing("").inside(
        "exec cat",
        0,
        Box::leak(token_env("sbx-cs-example").into_boxed_str()),
        "",
    );
    configure_token_env(&host, "sbxm-example", "sbx-cs-example")
        .required_because("the file already matches")?;
    assert_eq!(
        host.calls.borrow().len(),
        1,
        "read once, never written: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn a_token_env_file_sbxm_wrote_earlier_is_brought_up_to_the_current_placeholder() -> Checked {
    // tokenを登録し直すとplaceholderが変わる。古い値のままでは`gh`が401になる。
    let host = FakeSbx::listing("").inside(
        "exec cat",
        0,
        Box::leak(token_env("sbx-cs-old").into_boxed_str()),
        "",
    );
    configure_token_env(&host, "sbxm-example", "sbx-cs-new")
        .required_because("a file sbxm wrote is updated, not refused")?;
    assert!(
        host.calls
            .borrow()
            .iter()
            .any(|call| call.join(" ").contains("export GH_TOKEN=sbx-cs-new")),
        "the current placeholder is written: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn a_token_env_file_with_an_added_setting_is_never_replaced() -> Checked {
    let content = format!(
        "{}export ANOTHER_SETTING=keep-me\n",
        token_env("sbx-cs-example")
    );
    let host = FakeSbx::listing("").inside("exec cat", 0, Box::leak(content.into_boxed_str()), "");
    let error = configure_token_env(&host, "sbxm-example", "sbx-cs-example")
        .refused_because("the marker does not grant ownership of added lines")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxTokenEnvUnusable));
    assert_eq!(
        host.calls.borrow().len(),
        1,
        "the added setting is not discarded: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn a_token_env_file_sbxm_did_not_write_is_refused() -> Checked {
    // 同じ名前で誰かが置いたfileを上書きしない。
    let host =
        FakeSbx::listing("").inside("exec cat", 0, "export GH_TOKEN=someone-elses-value\n", "");
    let error = configure_token_env(&host, "sbxm-example", "sbx-cs-example")
        .refused_because("a file with unknown content is not overwritten")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxTokenEnvUnusable));
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("cause-token-env-unexpected-content"),
        "the diagnosis classifies the refusal without repeating the content: {diagnostic}"
    );
    assert!(
        !diagnostic.contains("someone-elses-value"),
        "credential file content never reaches the diagnosis: {diagnostic}"
    );
    assert_eq!(
        host.calls.borrow().len(),
        1,
        "nothing is written after the refusal: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn an_empty_token_env_file_is_treated_as_absent_and_written() -> Checked {
    // 正常に読めた空fileには残すべきものが無い。
    let host = FakeSbx::listing("").inside("exec cat", 0, "", "");
    configure_token_env(&host, "sbxm-example", "sbx-cs-example")
        .required_because("an empty file is not a foreign one")?;
    assert!(
        host.calls
            .borrow()
            .iter()
            .any(|call| call.join(" ").contains("printf")),
        "the file is written: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn a_missing_token_env_file_is_written() -> Checked {
    let host = FakeSbx::listing("").inside("exec cat", 44, "", "");
    configure_token_env(&host, "sbxm-example", "sbx-cs-example")
        .required_because("the probe confirmed the file is absent")?;
    assert!(
        host.calls
            .borrow()
            .iter()
            .any(|call| call.join(" ").contains("printf")),
        "the missing file is written: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn a_token_env_file_that_cannot_be_read_is_never_overwritten_as_root() -> Checked {
    // 空stdoutだけでは不在を証明できない。通常userのcatがpermissionで拒まれても、rootの
    // 書き込みは通るため、ここから上書きへ進むと未確認のfileを切り詰めてしまう。
    let host = FakeSbx::listing("").inside(
        "exec cat",
        1,
        "",
        "cat: /etc/profile.d/sbxm-github-token.sh: Permission denied\n",
    );
    let error = configure_token_env(&host, "sbxm-example", "sbx-cs-example")
        .refused_because("a failed read cannot authorize an overwrite")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxTokenEnvUnusable));
    assert!(
        format!("{error:?}").contains("cause-token-env-unreadable"),
        "the diagnosis distinguishes a failed read from unexpected content: {error:?}"
    );
    assert_eq!(
        host.calls.borrow().len(),
        1,
        "nothing is written after the failed read: {:?}",
        host.calls.borrow()
    );
    Ok(())
}

#[test]
fn a_partial_token_env_read_never_reaches_the_diagnosis() -> Checked {
    let secret = "export GH_TOKEN=ghp_audit_fixture_only\n";
    let host = FakeSbx::listing("").inside("exec cat", 1, secret, "Input/output error\n");
    let error = configure_token_env(&host, "sbxm-example", "sbx-cs-example")
        .refused_because("a partial read is still unobservable")?;
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("cause-token-env-unreadable"),
        "the failed read is classified: {diagnostic}"
    );
    assert!(
        !diagnostic.contains("ghp_audit_fixture_only"),
        "partially read credential material is not exposed: {diagnostic}"
    );
    Ok(())
}

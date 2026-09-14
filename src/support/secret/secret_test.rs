use crate::boundary::host::{CommandSpec, HostEnvironment};
use crate::diagnostics::{ErrorId, Result};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::boundary::host::CommandOutcome;
use crate::project::ProjectId;
use crate::testing::host::{custom_secret_listing, no_secrets_listing, service_secret_listing};
use std::cell::RefCell;

struct FakeSbx {
    /// `secret ls`へ順に返す出力。末尾から取り出し、最後の1件は繰り返す。
    listings: RefCell<Vec<String>>,
    calls: RefCell<Vec<Vec<String>>>,
    /// `git ls-remote`が返すexit statusとstderr。
    ls_remote: (i32, &'static str),
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
            ls_remote: (0, ""),
        }
    }

    fn refusing_ls_remote(stderr: &'static str) -> FakeSbx {
        FakeSbx {
            ls_remote: (128, stderr),
            ..FakeSbx::listing("")
        }
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.args.clone());
        if spec.args.iter().any(|arg| arg == "ls-remote") {
            let (code, stderr) = self.ls_remote;
            let mut outcome = crate::testing::command::outcome(spec, code, "");
            outcome.stderr = stderr.as_bytes().to_vec();
            return Ok(outcome);
        }
        let mut listings = self.listings.borrow_mut();
        let output = listings.last().cloned().unwrap_or_default();
        // 一覧を読み直す工程だけが次の観測へ進む。最後の1件は繰り返す。
        let reads_listing = spec.args.first().is_some_and(|arg| arg == "secret")
            && spec.args.get(1).is_some_and(|arg| arg == "ls");
        if reads_listing && listings.len() > 1 {
            listings.pop();
        }
        Ok(crate::testing::command::outcome(spec, 0, &output))
    }
}

fn registered() -> String {
    service_secret_listing("sbxm-example")
}

fn none() -> String {
    no_secrets_listing()
}

fn project() -> Checked<ProjectId> {
    ProjectId::parse("example-org/example-repo").required()
}

#[test]
fn a_registered_service_secret_lets_the_build_continue() -> Checked {
    let host = FakeSbx::listing(&registered());
    require_github(&host, "sbxm-example").required_because("the secret is there")?;

    let calls = host.calls.borrow();
    assert_eq!(
        calls[0],
        vec!["secret".to_string(), "ls".to_string(), "--json".to_string()],
        "the check is read-only and never asks for the value"
    );
    Ok(())
}

#[test]
fn a_global_service_secret_counts_for_every_sandbox() -> Checked {
    let host = FakeSbx::listing(&service_secret_listing("(global)"));
    require_github(&host, "sbxm-example")
        .required_because("a global registration reaches this sandbox too")?;
    Ok(())
}

#[test]
fn a_service_secret_of_another_sandbox_does_not_count() -> Checked {
    let host = FakeSbx::listing(&service_secret_listing("sbxm-other"));
    let error = require_github(&host, "sbxm-example")
        .refused_because("a token scoped to another sandbox never reaches this one")?;
    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
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
    assert_eq!(
        remediation
            .commands
            .iter()
            .map(|command| command.as_str().to_string())
            .collect::<Vec<_>>(),
        vec![register_command("sbxm-example")]
    );
    Ok(())
}

#[test]
fn the_command_sbxm_prints_registers_for_the_sandbox_and_asks_for_the_token_interactively() {
    // 案内と検査がずれると、案内どおりに実行しても止まり続ける状態になる。
    let command = register_command("sbxm-example");
    assert_eq!(command, "sbx secret set github --sandbox sbxm-example");
    assert!(
        !command.contains("--token") && !command.contains("--value"),
        "the token is asked for interactively and never placed on the command line: {command}"
    );
}

#[test]
fn a_custom_secret_the_previous_release_asked_for_is_explained_and_cleaned_up() -> Checked {
    // 以前の版の案内で登録したcustom secretは、組み込みserviceに影にされて届かない。
    // 登録してあるのに動かない理由を示し、消すcommandと登録し直すcommandを並べる。
    let host = FakeSbx::listing(&custom_secret_listing("sbxm-example", "sbx-cs-legacy"));
    let error = require_github(&host, "sbxm-example")
        .refused_because("a shadowed custom secret does not reach the sandbox")?;

    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    let diagnostic = &error.diagnostics()[0];
    let rendered = format!("{diagnostic:?}");
    assert!(
        rendered.contains("cause-github-custom-secret-shadowed"),
        "the collision with the built-in service is named: {rendered}"
    );
    let remediation = diagnostic
        .remediation
        .as_ref()
        .required_because("the user is told how to get out of it")?;
    assert_eq!(
        remediation
            .commands
            .iter()
            .map(|command| command.as_str().to_string())
            .collect::<Vec<_>>(),
        vec![
            forget_custom_command("sbxm-example", "sbx-cs-legacy"),
            register_command("sbxm-example"),
        ]
    );
    Ok(())
}

#[test]
fn a_shadowed_custom_secret_of_another_scope_is_explained_but_not_removed() -> Checked {
    // global scopeのcustom secretはほかのSandboxも持っている。理由としては示すが、
    // この案件の対処で消すcommandは出さない。
    let host = FakeSbx::listing(&custom_secret_listing("(global)", "sbx-cs-elsewhere"));
    let error = require_github(&host, "sbxm-example").refused_because("nothing reaches it")?;
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("present")?;
    assert!(
        remediation
            .commands
            .iter()
            .all(|command| !command.as_str().contains("--placeholder")),
        "a registration this project does not own is not named for removal: {remediation:?}"
    );
    assert!(format!("{error:?}").contains("cause-github-custom-secret-shadowed"));
    Ok(())
}

#[test]
fn the_registration_is_removed_and_verified_gone() -> Checked {
    let host = FakeSbx::listings(&[&registered(), &none()]);
    forget_github(&host, "sbxm-example").required_because("the registration is removed")?;

    let calls = host.calls.borrow();
    assert_eq!(
        format!("sbx {}", calls[1].join(" ")),
        forget_command("sbxm-example"),
        "sbxm runs exactly the command it names when the removal has to be repeated by hand"
    );
    assert!(
        calls[1].contains(&"--sandbox".to_string()),
        "the scope is given, so the global registration is never the target: {calls:?}"
    );
    assert_eq!(
        calls.len(),
        3,
        "the listing is read again, so absence is observed instead of assumed: {calls:?}"
    );
    Ok(())
}

#[test]
fn a_custom_secret_the_previous_release_registered_is_removed_with_the_sandbox() -> Checked {
    // 以前の版の登録が残っていると、存在しないSandbox宛のtokenを預けたままになる。
    let host = FakeSbx::listings(&[
        &custom_secret_listing("sbxm-example", "sbx-cs-legacy"),
        &none(),
    ]);
    forget_github(&host, "sbxm-example").required_because("the legacy registration is removed")?;
    assert_eq!(
        format!("sbx {}", host.calls.borrow()[1].join(" ")),
        forget_custom_command("sbxm-example", "sbx-cs-legacy")
    );
    Ok(())
}

#[test]
fn a_registration_of_another_scope_is_not_removed_with_this_sandbox() -> Checked {
    // global scopeのsecretはほかのSandboxも使う。1案件の後片付けで消す対象ではない。
    let host = FakeSbx::listing(&service_secret_listing("(global)"));
    forget_github(&host, "sbxm-example").required_because("nothing of this project is there")?;
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
fn nothing_is_removed_when_no_token_was_ever_registered() -> Checked {
    let host = FakeSbx::listing(&none());
    forget_github(&host, "sbxm-example")
        .required_because("an unregistered scope is not an error")?;
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
            .any(|command| command.as_str() == forget_command("sbxm-example"))
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
            "sbxm only asks whether the secret exists: {args:?}"
        );
    }
    Ok(())
}

#[test]
fn a_sandbox_that_carries_the_token_variable_passes_the_check() -> Checked {
    let host = FakeSbx::listing("gho_sbxproxymanaged000000000000000000000");
    require_token_env_present(&host, "sbxm-example").required_because("the variable is there")?;
    Ok(())
}

#[test]
fn a_sandbox_without_the_token_variable_is_refused_instead_of_assumed_ready() -> Checked {
    // 登録済みという事実からSandboxへ届いたと推定しない。
    let host = FakeSbx::listing("");
    let error = require_token_env_present(&host, "sbxm-example")
        .refused_because("git inside cannot authenticate without the variable")?;

    assert_eq!(error.first_id(), Some(ErrorId::SandboxSecretNotApplied));
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("the user is told how to get out of it")?;
    assert!(
        remediation
            .commands
            .iter()
            .any(|command| command.as_str() == register_command("sbxm-example")),
        "a sandbox-scoped registration takes effect at once, so it is what is offered: {remediation:?}"
    );
    Ok(())
}

#[test]
fn github_accepting_the_credential_lets_the_build_continue() -> Checked {
    let host = FakeSbx::listing("");
    require_github_accepts(&host, "sbxm-example", &project()?)
        .required_because("GitHub answered the probe")?;
    let calls = host.calls.borrow();
    let probe = calls[0].join(" ");
    assert!(
        probe.contains("git ls-remote https://github.com/example-org/example-repo.git HEAD"),
        "the probe takes the same path as the fetch that follows: {probe}"
    );
    Ok(())
}

#[test]
fn a_credential_github_rejects_is_named_before_the_fetch_with_how_to_register_again() -> Checked {
    // tokenが無効でも、sentinelが差し替えられずそのまま届いても、GitHubは同じ文で拒む。
    for stderr in [
        "remote: Invalid username or token. Password authentication is not supported for Git operations.\nfatal: Authentication failed for 'https://github.com/example-org/example-repo.git/'\n",
        "fatal: could not read Username for 'https://github.com': No such device or address\n",
    ] {
        let host = FakeSbx::refusing_ls_remote(stderr);
        let error = require_github_accepts(&host, "sbxm-example", &project()?)
            .refused_because("a rejected credential stops the build before the fetch")?;
        assert_eq!(error.first_id(), Some(ErrorId::GithubCredentialRejected));
        let remediation = error.diagnostics()[0]
            .remediation
            .as_ref()
            .required_because("the user is told how to register the token again")?;
        assert_eq!(
            remediation
                .commands
                .iter()
                .map(|command| command.as_str().to_string())
                .collect::<Vec<_>>(),
            vec![
                forget_command("sbxm-example"),
                register_command("sbxm-example")
            ]
        );
    }
    Ok(())
}

#[test]
fn a_failure_that_is_not_a_rejection_is_reported_as_the_command_that_failed() -> Checked {
    // 到達できない・repositoryが無いなどは、tokenを登録し直しても直らない。
    let host = FakeSbx::refusing_ls_remote(
        "fatal: unable to access 'https://github.com/example-org/example-repo.git/': Could not resolve host: github.com\n",
    );
    let error = require_github_accepts(&host, "sbxm-example", &project()?)
        .refused_because("the probe failed for another reason")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}

#[test]
fn the_credential_helper_reads_the_token_variable_and_holds_no_token() -> Checked {
    let host = FakeSbx::listing("");
    configure_git_credential(&host, "sbxm-example").required_because("the helper is configured")?;

    let calls = host.calls.borrow();
    assert!(
        calls[0]
            .join(" ")
            .contains("credential.https://github.com.helper"),
        "the current value is read before anything is written: {calls:?}"
    );
    let write = calls
        .iter()
        .map(|call| call.join(" "))
        .find(|call| call.contains("password=$GH_TOKEN"))
        .required_because("the helper is configured because nothing was set yet")?;
    // helperはSandboxの環境変数を読むだけで、値そのものは持たない。
    assert!(write.contains("credential.https://github.com.helper"));
    Ok(())
}

#[test]
fn configure_leaves_an_existing_matching_credential_helper_untouched() -> Checked {
    let key = "exec sbxm-example -- git config --global --get credential.https://github.com.helper";
    let helper = "!f() { echo username=x; echo password=$GH_TOKEN; }; f";
    let host = crate::testing::host::FakeSbx::listing("").answering(key, 0, helper);
    configure_git_credential(&host, "sbxm-example").required_because("the helper is configured")?;
    assert!(
        !host.ran("credential.https://github.com.helper !f"),
        "an already-matching value is not written again: {:?}",
        host.calls()
    );
    assert_eq!(
        host.calls().len(),
        1,
        "the value is read once and never written: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn configure_refuses_a_different_credential_helper() -> Checked {
    let key = "exec sbxm-example -- git config --global --get credential.https://github.com.helper";
    let host = crate::testing::host::FakeSbx::listing("").answering(key, 0, "store");
    let error = configure_git_credential(&host, "sbxm-example")
        .refused_because("a helper set to something else may belong to another user's sandbox")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::SandboxCredentialHelperUnusable)
    );
    assert_eq!(
        host.calls().len(),
        1,
        "the mismatch is refused before any write is attempted: {:?}",
        host.calls()
    );
    assert!(
        !host.ran("credential.https://github.com.helper !f"),
        "the existing value is not overwritten: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn configure_refuses_a_credential_helper_it_cannot_observe() -> Checked {
    let key = "exec sbxm-example -- git config --global --get credential.https://github.com.helper";
    let host = crate::testing::host::FakeSbx::listing("").answering(key, 1, "fatal: bad config");
    let error = configure_git_credential(&host, "sbxm-example")
        .refused_because("a failure that answered with text is not the same as an unset key")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::SandboxCredentialHelperUnusable)
    );
    assert_eq!(
        host.calls().len(),
        1,
        "nothing is written while the existing value cannot be read: {:?}",
        host.calls()
    );
    Ok(())
}

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::{ErrorId, Result};
use crate::metadata::GitIdentity;
use crate::msg;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::testing::sandbox::InnerCommandSandbox;
use std::cell::RefCell;
use std::collections::HashMap;

struct FakeSbx {
    /// Sandbox内で既に設定されている値。
    settings: HashMap<String, String>,
    calls: RefCell<Vec<Vec<String>>>,
}

impl FakeSbx {
    fn holding(settings: &[(&str, &str)]) -> FakeSbx {
        FakeSbx {
            settings: settings
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect(),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn stored(&self, key: &str) -> (i32, String) {
        match self.settings.get(key) {
            Some(value) => (0, format!("{value}\n")),
            None => (1, String::new()),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }

    fn wrote(&self, value: &str) -> bool {
        self.calls()
            .iter()
            .any(|args| args.iter().any(|arg| arg == value))
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.args.clone());
        let inner = crate::testing::command::inner_args(spec);

        let (code, stdout) = match inner.as_slice() {
            ["git", "config", "--global", "--get", key] => self.stored(key),
            ["gh", "config", "get", "git_protocol", ..] => self.stored("git_protocol"),
            _ => (0, String::new()),
        };

        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}

fn identity() -> GitIdentity {
    GitIdentity {
        user_name: "Example User".into(),
        user_email: "user@example.com".into(),
    }
}

#[test]
fn an_unset_sandbox_is_configured_from_the_global_configuration() -> Checked {
    let host = FakeSbx::holding(&[]);
    ensure(&host, "sbxm-example", &identity()).required_because("configure")?;

    assert!(host.wrote("Example User"));
    assert!(host.wrote("user@example.com"));
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.iter().any(|arg| arg == "gh")),
        "gh belongs to the tool listing, not to the git identity: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn the_protocol_is_written_only_when_it_is_not_already_https() -> Checked {
    let host = FakeSbx::holding(&[]);
    ensure_git_protocol(&host, "sbxm-example").required_because("configure")?;
    assert!(
        host.calls()
            .iter()
            .any(|args| args.contains(&"set".to_string())
                && args.contains(&GIT_PROTOCOL.to_string()))
    );

    // ghの既定は`https`である。一致を観測したSandboxへは書き込まない。
    let host = FakeSbx::holding(&[("git_protocol", GIT_PROTOCOL)]);
    ensure_git_protocol(&host, "sbxm-example").required_because("nothing to do")?;
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.contains(&"set".to_string())),
        "{:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_protocol_that_was_changed_by_hand_stops_the_run() -> Checked {
    let host = FakeSbx::holding(&[("git_protocol", "ssh")]);
    let error = ensure_git_protocol(&host, "sbxm-example")
        .refused_because("a different value is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxIdentityMismatch));
    Ok(())
}

#[test]
fn a_protocol_that_could_not_be_asked_about_is_not_written_over() -> Checked {
    // 問い合わせが届かなかったことを「未設定」と読むと、`ssh`のまま残ったSandboxを
    // 書き換えたつもりで先へ進む。observationが無いことは空の設定ではない。
    let host = InnerCommandSandbox::new()
        .timing_out(&format!("gh config get git_protocol --host {GITHUB_HOST}"));

    let error = ensure_git_protocol(&host, "sbxm-example")
        .refused_because("a question that went unanswered is not an answer")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(
        !host.ran("config set"),
        "nothing is written after a question that went unanswered: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_protocol_that_could_not_be_written_stops_the_run() -> Checked {
    // 書き込めたことを確かめずに進むと、HTTPSを設定したという前提のまま
    // cloneへ入る。
    let host = InnerCommandSandbox::new().timing_out(&format!(
        "gh config set git_protocol {GIT_PROTOCOL} --host {GITHUB_HOST}"
    ));

    let error = ensure_git_protocol(&host, "sbxm-example")
        .refused_because("a setting that was not written is not a setting")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    Ok(())
}

#[test]
fn values_that_already_match_are_left_as_they_are() -> Checked {
    let host = FakeSbx::holding(&[
        ("user.name", "Example User"),
        ("user.email", "user@example.com"),
    ]);
    ensure(&host, "sbxm-example", &identity()).required_because("nothing to do")?;

    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.contains(&"set".to_string())),
        "an already configured sandbox is not written to: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn read_only_identity_observation_reports_missing_empty_matching_and_other_values() -> Checked {
    let expected = identity();
    assert!(observe(
        &FakeSbx::holding(&[
            ("user.name", "Example User"),
            ("user.email", "user@example.com"),
        ]),
        "sbxm-example",
        &expected,
    )?);
    assert!(!observe(&FakeSbx::holding(&[]), "sbxm-example", &expected)?);
    assert!(!observe(
        &FakeSbx::holding(&[("user.name", ""), ("user.email", "user@example.com")]),
        "sbxm-example",
        &expected,
    )?);

    let error = observe(
        &FakeSbx::holding(&[("user.name", "Another User")]),
        "sbxm-example",
        &expected,
    )
    .refused_because("a different identity is not silently accepted")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxIdentityMismatch));
    Ok(())
}

#[test]
fn read_only_git_protocol_observation_reports_missing_empty_matching_and_other_values() -> Checked {
    assert!(observe_git_protocol(
        &FakeSbx::holding(&[("git_protocol", "https")]),
        "sbxm-example",
    )?);
    assert!(!observe_git_protocol(
        &FakeSbx::holding(&[]),
        "sbxm-example",
    )?);
    assert!(!observe_git_protocol(
        &FakeSbx::holding(&[("git_protocol", "")]),
        "sbxm-example",
    )?);

    let error = observe_git_protocol(
        &FakeSbx::holding(&[("git_protocol", "ssh")]),
        "sbxm-example",
    )
    .refused_because("a different protocol is not silently accepted")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxIdentityMismatch));
    Ok(())
}

#[test]
fn a_sandbox_configured_for_someone_else_is_not_overwritten() -> Checked {
    for settings in [
        vec![("user.name", "Another User")],
        vec![
            ("user.name", "Example User"),
            ("user.email", "another@example.com"),
        ],
    ] {
        let host = FakeSbx::holding(&settings);
        let error = ensure(&host, "sbxm-example", &identity())
            .refused_because("a different value belongs to someone else")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::SandboxIdentityMismatch),
            "{settings:?} produced the wrong error"
        );
        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.contains(&"Example User".to_string())
                    && args.contains(&"--global".to_string())
                    && !args.contains(&"--get".to_string())),
            "nothing is written while a value disagrees: {:?}",
            host.calls()
        );
    }
    Ok(())
}

/// hostの`git config --global --get-all`が返す原文を決め打ちするhost。
struct FakeGitConfig {
    answers: HashMap<String, (i32, String)>,
}

impl FakeGitConfig {
    fn answering(pairs: &[(&str, i32, &str)]) -> FakeGitConfig {
        FakeGitConfig {
            answers: pairs
                .iter()
                .map(|(key, code, stdout)| ((*key).to_string(), (*code, (*stdout).to_string())))
                .collect(),
        }
    }
}

impl HostEnvironment for FakeGitConfig {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        let (code, stdout) = match args.as_slice() {
            ["config", "--global", "--get-all", key] => self
                .answers
                .get(*key)
                .cloned()
                .unwrap_or((1, String::new())),
            _ => (1, String::new()),
        };
        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}

#[test]
fn the_candidate_is_what_git_declares() {
    let host = FakeGitConfig::answering(&[
        ("user.name", 0, "Example User\n"),
        ("user.email", 0, "  user@example.com  \n"),
    ]);
    assert_eq!(
        candidate_from_host(&host, "user.name"),
        identity().user_name
    );
    assert_eq!(
        candidate_from_host(&host, "user.email"),
        identity().user_email
    );
}

#[test]
fn a_declaration_that_cannot_be_reduced_to_one_value_offers_no_candidate() {
    for (code, stdout) in [
        // 未設定。
        (1, ""),
        // 複数値。空の宣言も1つの宣言であり、落として1件に見せない。
        (0, "Example User\nOther User\n"),
        (0, "Example User\n\n"),
        (0, "\nExample User\n"),
        // 空、または改行だけの値。
        (0, "   \n"),
    ] {
        let host = FakeGitConfig::answering(&[("user.name", code, stdout)]);
        assert_eq!(
            candidate_from_host(&host, "user.name"),
            "",
            "({code}, {stdout:?}) must offer no candidate"
        );
    }
}

#[test]
fn a_host_that_cannot_be_observed_offers_no_candidate_rather_than_failing() {
    // 候補は決定ではない。読めないことを案件を止める理由にしない。
    let host = UnobservableHost;
    assert_eq!(candidate_from_host(&host, "user.name"), "");
    assert_eq!(candidate_from_host(&host, "user.email"), "");
}

/// `git`そのものを実行できないhost。
struct UnobservableHost;

impl HostEnvironment for UnobservableHost {
    fn command_exists(&self, _program: &str) -> bool {
        false
    }

    fn run(&self, _spec: &CommandSpec) -> Result<CommandOutcome> {
        Err(crate::diagnostics::Error::new(
            ErrorId::HostCommandMissing,
            msg!("error-host-command-missing", program = "git"),
        ))
    }
}

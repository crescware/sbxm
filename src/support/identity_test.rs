use super::*;
use crate::command::{CommandOutcome, CommandSpec};
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
                .map(|(key, value)| (key.to_string(), value.to_string()))
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
fn an_unset_sandbox_is_configured_from_the_global_configuration() {
    let host = FakeSbx::holding(&[]);
    ensure(&host, "sbxm-example", &identity()).expect("configure");

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
}

#[test]
fn the_protocol_is_written_only_when_it_is_not_already_https() {
    let host = FakeSbx::holding(&[]);
    ensure_git_protocol(&host, "sbxm-example").expect("configure");
    assert!(
        host.calls()
            .iter()
            .any(|args| args.contains(&"set".to_string())
                && args.contains(&GIT_PROTOCOL.to_string()))
    );

    // ghの既定は`https`である。一致を観測したSandboxへは書き込まない。
    let host = FakeSbx::holding(&[("git_protocol", GIT_PROTOCOL)]);
    ensure_git_protocol(&host, "sbxm-example").expect("nothing to do");
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.contains(&"set".to_string())),
        "{:?}",
        host.calls()
    );
}

#[test]
fn a_protocol_that_was_changed_by_hand_stops_the_run() {
    let host = FakeSbx::holding(&[("git_protocol", "ssh")]);
    let error =
        ensure_git_protocol(&host, "sbxm-example").expect_err("a different value is refused");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxIdentityMismatch));
}

#[test]
fn values_that_already_match_are_left_as_they_are() {
    let host = FakeSbx::holding(&[
        ("user.name", "Example User"),
        ("user.email", "user@example.com"),
    ]);
    ensure(&host, "sbxm-example", &identity()).expect("nothing to do");

    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.contains(&"set".to_string())),
        "an already configured sandbox is not written to: {:?}",
        host.calls()
    );
}

#[test]
fn a_sandbox_configured_for_someone_else_is_not_overwritten() {
    for settings in [
        vec![("user.name", "Another User")],
        vec![
            ("user.name", "Example User"),
            ("user.email", "another@example.com"),
        ],
    ] {
        let host = FakeSbx::holding(&settings);
        let error = ensure(&host, "sbxm-example", &identity())
            .expect_err("a different value belongs to someone else");
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
fn the_host_identity_is_read_from_what_git_declares() {
    let host = FakeGitConfig::answering(&[
        ("user.name", 0, "Example User\n"),
        ("user.email", 0, "  user@example.com  \n"),
    ]);
    assert_eq!(
        from_host(&host).expect("the host declares both"),
        identity()
    );
}

#[test]
fn a_declaration_that_cannot_be_reduced_to_one_value_is_refused() {
    for (name, email) in [
        // 未設定。
        ((1, ""), (0, "user@example.com\n")),
        ((0, "Example User\n"), (1, "")),
        // 複数値。空の宣言も1つの宣言であり、落として1件に見せない。
        ((0, "Example User\nOther User\n"), (0, "user@example.com\n")),
        ((0, "Example User\n\n"), (0, "user@example.com\n")),
        ((0, "\nExample User\n"), (0, "user@example.com\n")),
        // 空、または改行だけの値。
        ((0, "   \n"), (0, "user@example.com\n")),
    ] {
        let host = FakeGitConfig::answering(&[
            ("user.name", name.0, name.1),
            ("user.email", email.0, email.1),
        ]);
        let error = from_host(&host).expect_err("{name:?} {email:?} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::GitIdentityUnavailable),
            "{name:?} {email:?} produced the wrong error"
        );
    }
}

use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment, TimeoutClass};
use crate::diagnostics::{ErrorId, Result};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

/// `sbx exec`へ、決め打ちのstdoutと終了statusで答えるhost。
struct Answering {
    stdout: Vec<u8>,
    code: i32,
}

impl HostEnvironment for Answering {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        Ok(crate::testing::command::outcome(
            spec,
            self.code,
            &String::from_utf8_lossy(&self.stdout),
        ))
    }
}

fn answering(stdout: &[u8], code: i32) -> Answering {
    Answering {
        stdout: stdout.to_vec(),
        code,
    }
}

/// 隔離領域に、受け取り途中のfileが残っていないか。
fn leftovers(directory: &std::path::Path) -> Checked<Vec<String>> {
    Ok(fs::read_dir(directory)
        .required()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect())
}

#[test]
fn what_the_sandbox_wrote_arrives_as_a_private_file_in_the_isolated_area() -> Checked {
    let root = tempfile::tempdir().required()?;
    let incoming = root.path().join("incoming");

    let path = receive(
        &answering(b"sandbox bytes", 0),
        "sbxm-example",
        &["cat", "/home/agent/.gitconfig"],
        &incoming,
        "gitconfig",
        1024,
        TimeoutClass::SandboxLifecycle,
    )
    .required()?;

    assert_eq!(path, incoming.join("gitconfig"));
    assert_eq!(fs::read(&path).required()?, b"sandbox bytes");
    assert_eq!(
        fs::metadata(&path).required()?.permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&incoming).required()?.permissions().mode() & 0o777,
        0o700
    );
    Ok(())
}

#[test]
fn a_failed_or_oversized_receipt_leaves_nothing_behind() -> Checked {
    let root = tempfile::tempdir().required()?;
    let incoming = root.path().join("incoming");

    let error = receive(
        &answering(b"partial", 1),
        "sbxm-example",
        &["cat", "/home/agent/.gitconfig"],
        &incoming,
        "gitconfig",
        1024,
        TimeoutClass::SandboxLifecycle,
    )
    .refused_because("the sandbox command failed")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert!(
        leftovers(&incoming)?.is_empty(),
        "{:?}",
        leftovers(&incoming)?
    );

    let error = receive(
        &answering(b"more than allowed", 0),
        "sbxm-example",
        &["cat", "/home/agent/.gitconfig"],
        &incoming,
        "gitconfig",
        4,
        TimeoutClass::SandboxLifecycle,
    )
    .refused_because("the output is larger than allowed")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputTooLarge)
    );
    assert!(
        leftovers(&incoming)?.is_empty(),
        "{:?}",
        leftovers(&incoming)?
    );
    Ok(())
}

#[test]
fn an_existing_file_is_never_overwritten() -> Checked {
    let root = tempfile::tempdir().required()?;
    let incoming = root.path().join("incoming");
    fs::create_dir(&incoming).required()?;
    fs::set_permissions(&incoming, fs::Permissions::from_mode(0o700)).required()?;
    fs::write(incoming.join("gitconfig"), b"kept").required()?;

    receive(
        &answering(b"sandbox bytes", 0),
        "sbxm-example",
        &["cat", "/home/agent/.gitconfig"],
        &incoming,
        "gitconfig",
        1024,
        TimeoutClass::SandboxLifecycle,
    )
    .refused_because("the name is already taken")?;
    assert_eq!(fs::read(incoming.join("gitconfig")).required()?, b"kept");
    assert_eq!(leftovers(&incoming)?, vec!["gitconfig".to_string()]);
    Ok(())
}

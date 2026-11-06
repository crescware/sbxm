use std::ffi::OsStr;

use crate::design::ExternalOutput;
use crate::testing::host::{FakeSbx, reusable_executable};
use crate::testing::outcome::{Checked, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::ReadOnly;
use crate::boundary::host::{CommandSpec, HostEnvironment, TerminalCommand};

#[test]
fn status_queries_disable_optional_git_writes_locally_and_inside_the_sandbox() {
    let spec = CommandSpec::probe("git", &["status", "--porcelain"]).read_only();
    let process = super::super::configure(&spec);
    let vars: Vec<_> = process.get_envs().collect();
    for (key, value) in [("GIT_OPTIONAL_LOCKS", "0"), ("GIT_NO_LAZY_FETCH", "1")] {
        assert!(vars.contains(&(OsStr::new(key), Some(OsStr::new(value)))));
    }
    for (args, inserted) in [
        (vec!["exec", "example", "--", "git", "status"], true),
        (vec!["exec", "example"], false),
        (vec!["ls", "--json"], false),
    ] {
        let spec = CommandSpec::probe("sbx", &args).read_only();
        let process = super::super::configure(&spec);
        let actual: Vec<_> = process
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        if inserted {
            assert_eq!(
                actual,
                [
                    "exec",
                    "example",
                    "--",
                    "env",
                    "GIT_OPTIONAL_LOCKS=0",
                    "GIT_NO_LAZY_FETCH=1",
                    "git",
                    "status"
                ]
            );
        } else {
            assert_eq!(actual, args);
        }
    }
    let ordinary = super::super::configure(&CommandSpec::probe("git", &["status"]));
    assert!(
        !ordinary
            .get_envs()
            .any(|(name, _)| name == "GIT_OPTIONAL_LOCKS")
    );
}

#[test]
fn read_only_execution_keeps_capabilities_and_marks_each_output_policy() -> Checked {
    let inner = FakeSbx::listing("[]").with_executable("git", reusable_executable("git"));
    let host = ReadOnly(&inner);
    assert!(host.command_exists("git"));
    assert!(host.inspect_executable("git").is_some());
    let spec = CommandSpec::probe("tool", &[]);
    let terminal = TerminalCommand::relayed("tool", &[]);
    let mut output = RecordedOutput::new();
    host.run(&spec).required()?;
    host.run_with_terminal(&terminal, &mut output).required()?;
    host.run_with_terminal_ticking(
        &terminal,
        &mut output,
        std::time::Duration::from_secs(1),
        &mut || {},
    )
    .required()?;
    host.run_streaming(&spec, &mut Vec::new(), 10).required()?;
    assert_eq!(inner.specs.borrow().len(), 4);
    assert!(inner.specs.borrow().iter().all(|spec| spec.read_only));
    assert!(!spec.read_only);
    assert!(!terminal.spec().read_only);
    output.finished();
    Ok(())
}

#[test]
fn real_git_status_leaves_a_stale_index_unchanged() -> Checked {
    use crate::testing::repository::git_in;

    let directory = tempfile::tempdir().required()?;
    git_in(directory.path(), &["init"])?;
    let file = directory.path().join("tracked");
    std::fs::write(&file, b"contents\n").required()?;
    git_in(directory.path(), &["add", "tracked"])?;
    git_in(directory.path(), &["commit", "-m", "initial"])?;
    let index = directory.path().join(".git/index");
    let before = std::fs::read(&index).required()?;
    std::fs::File::open(&file)
        .required()?
        .set_times(
            std::fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
            ),
        )
        .required()?;
    let host = ReadOnly(&crate::testing::sandbox::LocalSandbox);
    let spec = CommandSpec::probe("git", &["status", "--porcelain"]).working_dir(directory.path());
    assert!(host.run(&spec).required()?.stdout.is_empty());
    assert_eq!(std::fs::read(&index).required()?, before);
    // 同じ検査でも、通常のGitは古くなったstat情報をindexへ書き戻す。
    git_in(directory.path(), &["status", "--porcelain"])?;
    assert_ne!(std::fs::read(&index).required()?, before);
    Ok(())
}

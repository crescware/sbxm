use crate::diagnostics::{ErrorId, Result};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// 実行内容を記録するfake executableを作る。
fn fake_executable(dir: &Path, name: &str, body: &str) -> Checked<PathBuf> {
    let path = dir.join(name);
    {
        let mut file = fs::File::create(&path).required_because("create fake executable")?;
        file.write_all(format!("#!/bin/sh\n{body}\n").as_bytes())
            .required_because("write fake executable")?;
        file.sync_all().required_because("flush fake executable")?;
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .required_because("make executable")?;
    Ok(path)
}

/// 作りたてのfake executableを、timeout classの既定値で実行する。
fn run_fake(spec: &CommandSpec) -> Result<CommandOutcome> {
    retrying(|| run(spec))
}

/// spawnに失敗した試行だけをやり直す。
///
/// 別threadのtestがforkしている最中は、書き込み直後のfileが`ETXTBSY`で起動できない
/// ことがある。実装ではなくtest環境の競合なので、短い間だけ繰り返す。
fn retrying(attempt: impl Fn() -> Result<CommandOutcome>) -> Result<CommandOutcome> {
    for _ in 0..50 {
        match attempt() {
            Err(error) if error.contains_id(ErrorId::ExternalCommandSpawnFailed) => {
                std::thread::sleep(Duration::from_millis(10));
            }
            other => return other,
        }
    }
    attempt()
}

#[test]
fn timeout_classes_match_the_documented_defaults() {
    assert_eq!(
        TimeoutClass::Probe.duration(),
        Some(Duration::from_secs(10))
    );
    assert_eq!(
        TimeoutClass::LocalFilesystem.duration(),
        Some(Duration::from_secs(60))
    );
    // 長い工程ほど、途中で切ると成果物が中途半端に残る。
    assert!(TimeoutClass::SandboxLifecycle.duration() > TimeoutClass::LocalFilesystem.duration());
    assert!(TimeoutClass::ImageBuild.duration() > TimeoutClass::SandboxLifecycle.duration());
    // 対話接続を終える時期を決めるのは利用者である。
    assert_eq!(TimeoutClass::Interactive.duration(), None);
}

#[test]
fn a_command_runs_in_the_working_directory_it_was_given() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let workspace = dir.path().join("workspace");
    fs::create_dir(&workspace).required()?;
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(r#"pwd > "{}""#, record.display()),
    )?;

    let spec = CommandSpec::capture(fake.to_str().required()?, &[]).working_dir(&workspace);
    run_fake(&spec).required_because("the fake tool runs")?;

    let observed = fs::read_to_string(&record).required()?;
    assert_eq!(
        std::fs::canonicalize(observed.trim()).required()?,
        std::fs::canonicalize(&workspace).required()?
    );
    Ok(())
}

#[test]
fn passthrough_hands_the_streams_to_the_terminal_instead_of_capturing_them() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(
            r#"printf 'progress'; printf 'warning' >&2; printf 'ran' > "{}""#,
            record.display()
        ),
    )?;

    let spec = CommandSpec::passthrough(fake.to_str().required()?, &[]);
    let outcome = run_fake(&spec).required_because("the fake tool runs")?;

    assert_eq!(
        fs::read_to_string(&record).required()?,
        "ran",
        "the command still runs"
    );
    assert!(
        outcome.stdout.is_empty() && outcome.stderr.is_empty(),
        "passthrough output belongs to the terminal, not to a buffer"
    );
    assert!(!outcome.stderr_lossy);
    Ok(())
}

#[test]
fn a_failure_keeps_the_invocation_that_produced_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let workspace = dir.path().join("workspace");
    fs::create_dir(&workspace).required()?;
    let fake = fake_executable(dir.path(), "fake-tool", "exit 2")?;

    let spec = CommandSpec::capture(fake.to_str().required()?, &["clone", "--bare"])
        .working_dir(&workspace);
    let failure = run_fake(&spec).required_because("runs")?.failure();

    assert_eq!(failure.safe_args, vec!["clone", "--bare"]);
    assert_eq!(failure.working_dir.as_deref(), Some(workspace.as_path()));
    assert!(failure.exit_status.contains('2'));
    Ok(())
}

#[test]
fn every_argument_reaches_the_program_in_order() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(
            r#"for a in "$@"; do echo "arg=$a"; done > "{}""#,
            record.display()
        ),
    )?;

    let spec = CommandSpec::probe(fake.to_str().required()?, &["ls", "--json"]);
    let outcome = run_fake(&spec).required_because("the fake tool runs")?;
    assert!(outcome.success());

    assert_eq!(
        fs::read_to_string(&record).required()?,
        "arg=ls\narg=--json\n"
    );
    Ok(())
}

#[test]
fn arguments_are_passed_without_a_shell() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(r#"printf '%s' "$1" > "{}""#, record.display()),
    )?;

    // shellを介さないため、metacharacterはそのまま1個のargumentとして届く。
    let dangerous = "; rm -rf / #$(whoami)";
    let spec = CommandSpec::probe(fake.to_str().required()?, &[dangerous]);
    run_fake(&spec).required_because("the fake tool runs")?;

    assert_eq!(fs::read_to_string(&record).required()?, dangerous);
    Ok(())
}

#[test]
fn security_sensitive_runs_drop_the_ssh_agent_socket() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-sbx",
        &format!(
            r#"printf 'ssh=%s\n' "${{SSH_AUTH_SOCK-<unset>}}" > "{}""#,
            record.display()
        ),
    )?;

    // 親processのenvironmentは継承されるが、SSH_AUTH_SOCKだけは除外される。
    let inherited = CommandSpec::probe(fake.to_str().required()?, &[]);
    run_fake(&inherited).required_because("run with inherited environment")?;
    let with_agent = fs::read_to_string(&record).required()?;

    let stripped =
        CommandSpec::probe(fake.to_str().required()?, &[]).env(EnvPolicy::InheritWithoutSshAgent);
    run_fake(&stripped).required_because("run without the agent socket")?;
    let without_agent = fs::read_to_string(&record).required()?;

    assert_eq!(without_agent, "ssh=<unset>\n");
    // 親がSSH_AUTH_SOCKを持たない環境でも、除外側は常にunsetである。
    assert!(with_agent.starts_with("ssh="));
    Ok(())
}

#[test]
fn capture_keeps_both_streams_separately() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        "printf 'to stdout'; printf 'to stderr' >&2; exit 3",
    )?;

    let outcome =
        run_fake(&CommandSpec::probe(fake.to_str().required()?, &[])).required_because("runs")?;
    assert_eq!(outcome.stdout_text(), "to stdout");
    assert_eq!(outcome.failure().stderr_text(), "to stderr");
    assert!(!outcome.success());
    assert_eq!(outcome.status.code(), Some(3));
    Ok(())
}

#[test]
fn invalid_utf8_output_is_kept_as_bytes_and_reported_as_lossy() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let fake = fake_executable(dir.path(), "fake-tool", r"printf '\377\376' >&2")?;

    let outcome =
        run_fake(&CommandSpec::probe(fake.to_str().required()?, &[])).required_because("runs")?;
    assert_eq!(outcome.stderr, vec![0xff, 0xfe]);
    assert!(
        outcome.stderr_lossy,
        "a lossy conversion must be reported as such"
    );
    Ok(())
}

#[test]
fn a_command_that_exceeds_its_timeout_is_terminated() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let fake = fake_executable(dir.path(), "fake-tool", "sleep 30")?;

    let spec = CommandSpec::probe(fake.to_str().required()?, &[]);
    // probeの10秒を待たずに判定するため、直接短いdeadlineを使う。
    let started = Instant::now();
    let error = retrying(|| run_with_limit(&spec, Duration::from_millis(200)))
        .refused_because("the command must be terminated")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the child must be killed promptly"
    );
    Ok(())
}

#[test]
fn a_missing_program_is_distinguished_from_other_spawn_failures() -> Checked {
    let spec = CommandSpec::probe("sbxm-no-such-program-exists", &[]);
    let error = run(&spec).refused_because("missing programs fail")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotFound));
    Ok(())
}

#[test]
fn a_non_zero_status_maps_to_one_while_keeping_the_original_value() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let fake = fake_executable(dir.path(), "fake-tool", "printf 'boom' >&2; exit 42")?;

    let outcome =
        run_fake(&CommandSpec::probe(fake.to_str().required()?, &[])).required_because("runs")?;
    let error = outcome
        .require_success()
        .refused_because("a non-zero status is a failure")?;
    assert_eq!(error.exit_code(), crate::diagnostics::ExitCode::Failure);
    let diagnostic = &error.diagnostics()[0];
    let external = diagnostic
        .external
        .as_ref()
        .required_because("the original values are kept in the diagnostic")?;
    assert!(external.exit_status.contains("42"));
    assert_eq!(external.stderr_text(), "boom");
    Ok(())
}

#[test]
fn path_lookup_finds_an_executable_placed_at_the_front_of_path() -> Checked {
    let dir = tempfile::tempdir().required()?;
    fake_executable(dir.path(), "sbxm-fake-on-path", "exit 0")?;

    let original = std::env::var_os("PATH").unwrap_or_default();
    let mut entries = vec![dir.path().to_path_buf()];
    entries.extend(std::env::split_paths(&original));
    let joined = std::env::join_paths(entries).required_because("join PATH")?;

    // processのPATHは書き換えず、探索の対象となる値だけを渡す。
    assert!(
        exists_in_path_value("sbxm-fake-on-path", &joined),
        "an executable at the front of PATH must be found"
    );
    assert!(!exists_in_path_value("sbxm-fake-on-path", &original));
    assert!(!exists_on_path("sbxm-fake-on-path"));
    Ok(())
}

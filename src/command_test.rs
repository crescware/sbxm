use super::*;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// 実行内容を記録するfake executableを作る。
fn fake_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    {
        let mut file = fs::File::create(&path).expect("create fake executable");
        file.write_all(format!("#!/bin/sh\n{body}\n").as_bytes())
            .expect("write fake executable");
        file.sync_all().expect("flush fake executable");
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("make executable");
    path
}

/// 作りたてのfake executableを実行する。
///
/// 別threadのtestがforkしている最中は、書き込み直後のfileが`ETXTBSY`で起動できない
/// ことがある。実装ではなくtest環境の競合なので、短い間だけ再試行する。
fn run_fake(spec: &CommandSpec) -> Result<CommandOutcome> {
    run_fake_with_limit(spec, None)
}

/// `run_fake`と同じ再試行のうえで、timeout classの既定値ではない待ち時間を使う。
fn run_fake_with_limit(spec: &CommandSpec, limit: Option<Duration>) -> Result<CommandOutcome> {
    let attempt = || match limit {
        Some(limit) => run_with_limit(spec, limit),
        None => run(spec),
    };
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
fn a_command_runs_in_the_working_directory_it_was_given() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(r#"pwd > "{}""#, record.display()),
    );

    let spec = CommandSpec::capture(fake.to_str().unwrap(), &[]).working_dir(&workspace);
    run_fake(&spec).expect("the fake tool runs");

    let observed = fs::read_to_string(&record).unwrap();
    assert_eq!(
        std::fs::canonicalize(observed.trim()).unwrap(),
        std::fs::canonicalize(&workspace).unwrap()
    );
}

#[test]
fn passthrough_hands_the_streams_to_the_terminal_instead_of_capturing_them() {
    let dir = tempfile::tempdir().unwrap();
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(
            r#"printf 'progress'; printf 'warning' >&2; printf 'ran' > "{}""#,
            record.display()
        ),
    );

    let spec = CommandSpec::passthrough(fake.to_str().unwrap(), &[]);
    let outcome = run_fake(&spec).expect("the fake tool runs");

    assert_eq!(
        fs::read_to_string(&record).unwrap(),
        "ran",
        "the command still runs"
    );
    assert!(
        outcome.stdout.is_empty() && outcome.stderr.is_empty(),
        "passthrough output belongs to the terminal, not to a buffer"
    );
    assert!(!outcome.stderr_lossy);
}

#[test]
fn a_failure_keeps_the_invocation_that_produced_it() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let fake = fake_executable(dir.path(), "fake-tool", "exit 2");

    let spec =
        CommandSpec::capture(fake.to_str().unwrap(), &["clone", "--bare"]).working_dir(&workspace);
    let failure = run_fake(&spec).expect("runs").failure();

    assert_eq!(failure.safe_args, vec!["clone", "--bare"]);
    assert_eq!(failure.working_dir.as_deref(), Some(workspace.as_path()));
    assert!(failure.exit_status.contains('2'));
}

#[test]
fn every_argument_reaches_the_program_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(
            r#"for a in "$@"; do echo "arg=$a"; done > "{}""#,
            record.display()
        ),
    );

    let spec = CommandSpec::probe(fake.to_str().unwrap(), &["ls", "--json"]);
    let outcome = run_fake(&spec).expect("the fake tool runs");
    assert!(outcome.success());

    assert_eq!(fs::read_to_string(&record).unwrap(), "arg=ls\narg=--json\n");
}

#[test]
fn arguments_are_passed_without_a_shell() {
    let dir = tempfile::tempdir().unwrap();
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(r#"printf '%s' "$1" > "{}""#, record.display()),
    );

    // shellを介さないため、metacharacterはそのまま1個のargumentとして届く。
    let dangerous = "; rm -rf / #$(whoami)";
    let spec = CommandSpec::probe(fake.to_str().unwrap(), &[dangerous]);
    run_fake(&spec).expect("the fake tool runs");

    assert_eq!(fs::read_to_string(&record).unwrap(), dangerous);
}

#[test]
fn security_sensitive_runs_drop_the_ssh_agent_socket() {
    let dir = tempfile::tempdir().unwrap();
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-sbx",
        &format!(
            r#"printf 'ssh=%s\n' "${{SSH_AUTH_SOCK-<unset>}}" > "{}""#,
            record.display()
        ),
    );

    // 親processのenvironmentは継承されるが、SSH_AUTH_SOCKだけは除外される。
    let inherited = CommandSpec::probe(fake.to_str().unwrap(), &[]);
    run_fake(&inherited).expect("run with inherited environment");
    let with_agent = fs::read_to_string(&record).unwrap();

    let stripped =
        CommandSpec::probe(fake.to_str().unwrap(), &[]).env(EnvPolicy::InheritWithoutSshAgent);
    run_fake(&stripped).expect("run without the agent socket");
    let without_agent = fs::read_to_string(&record).unwrap();

    assert_eq!(without_agent, "ssh=<unset>\n");
    // 親がSSH_AUTH_SOCKを持たない環境でも、除外側は常にunsetである。
    assert!(with_agent.starts_with("ssh="));
}

#[test]
fn capture_keeps_both_streams_separately() {
    let dir = tempfile::tempdir().unwrap();
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        "printf 'to stdout'; printf 'to stderr' >&2; exit 3",
    );

    let outcome = run_fake(&CommandSpec::probe(fake.to_str().unwrap(), &[])).expect("runs");
    assert_eq!(outcome.stdout_text(), "to stdout");
    assert_eq!(outcome.failure().stderr_text(), "to stderr");
    assert!(!outcome.success());
    assert_eq!(outcome.status.code(), Some(3));
}

#[test]
fn invalid_utf8_output_is_kept_as_bytes_and_reported_as_lossy() {
    let dir = tempfile::tempdir().unwrap();
    let fake = fake_executable(dir.path(), "fake-tool", r#"printf '\377\376' >&2"#);

    let outcome = run_fake(&CommandSpec::probe(fake.to_str().unwrap(), &[])).expect("runs");
    assert_eq!(outcome.stderr, vec![0xff, 0xfe]);
    assert!(
        outcome.stderr_lossy,
        "a lossy conversion must be reported as such"
    );
}

#[test]
fn a_command_that_exceeds_its_timeout_is_terminated() {
    let dir = tempfile::tempdir().unwrap();
    let fake = fake_executable(dir.path(), "fake-tool", "sleep 30");

    let spec = CommandSpec::probe(fake.to_str().unwrap(), &[]);
    // probeの10秒を待たずに判定するため、直接短いdeadlineを使う。
    let started = Instant::now();
    let error = run_fake_with_limit(&spec, Some(Duration::from_millis(200)))
        .expect_err("the command must be terminated");
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the child must be killed promptly"
    );
}

#[test]
fn a_missing_program_is_distinguished_from_other_spawn_failures() {
    let spec = CommandSpec::probe("sbxm-no-such-program-exists", &[]);
    let error = run(&spec).expect_err("missing programs fail");
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotFound));
}

#[test]
fn a_non_zero_status_maps_to_one_while_keeping_the_original_value() {
    let dir = tempfile::tempdir().unwrap();
    let fake = fake_executable(dir.path(), "fake-tool", "printf 'boom' >&2; exit 42");

    let outcome = run_fake(&CommandSpec::probe(fake.to_str().unwrap(), &[])).expect("runs");
    let error = outcome
        .require_success()
        .expect_err("a non-zero status is a failure");
    assert_eq!(error.exit_code(), crate::error::ExitCode::Failure);
    let diagnostic = &error.diagnostics()[0];
    let external = diagnostic
        .external
        .as_ref()
        .expect("the original values are kept in the diagnostic");
    assert!(external.exit_status.contains("42"));
    assert_eq!(external.stderr_text(), "boom");
}

#[test]
fn path_lookup_finds_an_executable_placed_at_the_front_of_path() {
    let dir = tempfile::tempdir().unwrap();
    fake_executable(dir.path(), "sbxm-fake-on-path", "exit 0");

    let original = std::env::var_os("PATH").unwrap_or_default();
    let mut entries = vec![dir.path().to_path_buf()];
    entries.extend(std::env::split_paths(&original));
    let joined = std::env::join_paths(entries).expect("join PATH");

    // SAFETY: このtestだけがPATHを変更し、変更後も既存の全entryを保持する。
    unsafe { std::env::set_var("PATH", &joined) };
    let found = exists_on_path("sbxm-fake-on-path");
    unsafe { std::env::set_var("PATH", &original) };

    assert!(found, "an executable at the front of PATH must be found");
    assert!(!exists_on_path("sbxm-fake-on-path"));
}

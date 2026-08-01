use crate::design::Fact;
use crate::diagnostics::{ErrorId, Result};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use std::fs;
use std::io::{Read, Write};
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

/// timeout classの既定値ではない待ち時間で実行する。
///
/// 最短のclassでも10秒あるため、deadlineに達する側の分岐はtestからしか踏めない。
fn run_with_limit(spec: &CommandSpec, limit: Duration) -> Result<CommandOutcome> {
    run_inner(spec, Some(limit))
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

/// 読み出し1回分の結果。
enum Read1 {
    /// signalに中断された読み出し。
    Interrupted,
    Bytes(&'static [u8]),
    /// pipeそのものが読めなくなった。
    Failed,
    /// 読み出しの最中にthreadが落ちた。
    Crashed,
}

/// 決めた順に読み出しを返すfake stream。
///
/// 実際のpipeでは中断も失敗も起こる時期を選べないため、経路はこのfakeから踏む。
struct ScriptedPipe {
    reads: std::vec::IntoIter<Read1>,
}

impl ScriptedPipe {
    fn new(reads: Vec<Read1>) -> ScriptedPipe {
        ScriptedPipe {
            reads: reads.into_iter(),
        }
    }
}

impl Read for ScriptedPipe {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self.reads.next() {
            Some(Read1::Interrupted) => Err(std::io::ErrorKind::Interrupted.into()),
            Some(Read1::Bytes(bytes)) => {
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            Some(Read1::Failed) => Err(std::io::Error::other("the pipe could not be read")),
            // threadを落とす。`panic!`はlintが禁じているため、無い要素を読んで落とす。
            Some(Read1::Crashed) => {
                let nothing: Vec<usize> = Vec::new();
                Ok(nothing[0])
            }
            // 台本を使い切ったらEOFとする。
            None => Ok(0),
        }
    }
}

/// 診断が持つ事実の項目名。
fn labels(error: &crate::diagnostics::Error) -> Checked<Vec<String>> {
    Ok(error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .map(|fact| fact.label().id.to_string())
        .collect())
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
fn an_interactive_command_is_handed_the_terminal_and_waited_for_without_a_limit() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let record = dir.path().join("record");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(r#"printf 'ran' > "{}""#, record.display()),
    )?;

    let spec = CommandSpec::inherit(fake.to_str().required()?, &[]);
    // 終える時期を決めるのは利用者であり、sbxmは待ち切る。
    assert_eq!(spec.timeout.duration(), None);
    let outcome = run_fake(&spec).required_because("the fake tool runs")?;

    assert_eq!(fs::read_to_string(&record).required()?, "ran");
    assert!(outcome.success());
    assert!(
        outcome.stdout.is_empty() && outcome.stderr.is_empty(),
        "the terminal was handed over, so there is nothing to capture"
    );
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
fn a_timed_out_command_takes_the_processes_it_started_with_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let survivor = dir.path().join("survivor");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(
            // 背景のprocessは、待ち終えたらfileを残す。stdoutのpipeも握ったままである。
            r#"(sleep 2; printf 'alive' > "{}") &
sleep 30"#,
            survivor.display()
        ),
    )?;

    let spec = CommandSpec::probe(fake.to_str().required()?, &[]);
    let started = Instant::now();
    let error = retrying(|| run_with_limit(&spec, Duration::from_millis(200)))
        .refused_because("the command must be terminated")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    // 書き込み端を握るprocessが残っていれば、readerの回収はその分だけ返らない。
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "no pipe may stay open after the run returns"
    );

    std::thread::sleep(Duration::from_secs(3).saturating_sub(started.elapsed()));
    assert!(
        !survivor.exists(),
        "a process the command started must not outlive the command"
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
fn a_spawn_failure_that_is_not_a_missing_program_keeps_what_was_observed() -> Checked {
    let dir = tempfile::tempdir().required()?;

    // directoryは在るが起動できない。存在しないprogramとは別の失敗である。
    let spec = CommandSpec::probe(dir.path().to_str().required()?, &[]);
    let error = run(&spec).refused_because("a directory cannot be started")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    assert_eq!(
        labels(&error)?,
        vec!["diagnostic-command-label", "diagnostic-cause-label"],
        "the invocation and the reason the OS gave are both kept"
    );
    Ok(())
}

#[test]
fn an_interrupted_read_is_resumed_instead_of_ending_the_stream() -> Checked {
    let pipe = ScriptedPipe::new(vec![
        Read1::Interrupted,
        Read1::Bytes(b"out"),
        Read1::Interrupted,
        Read1::Bytes(b"put"),
    ]);

    let collected = spawn_reader(pipe)
        .join()
        .required_because("the reader ends")?
        .required_because("an interrupted read is not a failure")?;

    assert_eq!(collected, b"output");
    Ok(())
}

#[test]
fn a_stream_that_stops_being_readable_reports_it_instead_of_the_bytes_so_far() -> Checked {
    let pipe = ScriptedPipe::new(vec![Read1::Bytes(b"half"), Read1::Failed]);

    let error = spawn_reader(pipe)
        .join()
        .required_because("the reader ends")?
        .refused_because("the stream could not be read to the end")?;

    assert_eq!(error.kind(), std::io::ErrorKind::Other);
    Ok(())
}

#[test]
fn output_that_could_not_be_read_is_refused_rather_than_taken_as_empty() -> Checked {
    let spec = CommandSpec::probe("fake-tool", &[]);
    let reader = std::thread::spawn(|| Err(std::io::Error::other("the pipe could not be read")));

    let error = collect_reader(Some(reader), &spec)
        .refused_because("a stream that could not be read is not an empty stream")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    assert_eq!(
        labels(&error)?,
        vec!["diagnostic-command-label", "diagnostic-cause-label"]
    );
    Ok(())
}

#[test]
fn a_reader_that_ends_abnormally_is_refused_the_same_way() -> Checked {
    let spec = CommandSpec::probe("fake-tool", &[]);
    let reader = spawn_reader(ScriptedPipe::new(vec![
        Read1::Bytes(b"half"),
        Read1::Crashed,
    ]));

    let error = collect_reader(Some(reader), &spec)
        .refused_because("a reader that never finished read nothing")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnreadable)
    );
    // 原因はOSの原文ではなく、sbxm自身が観測したことなので翻訳する。
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?;
    assert!(
        diagnostic
            .facts
            .iter()
            .any(|fact| matches!(fact, Fact::Translated { .. })),
        "the reason sbxm observed is a translated fact"
    );
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

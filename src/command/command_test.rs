use crate::diagnostics::{ErrorId, Result};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

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
fn retrying(mut attempt: impl FnMut() -> Result<CommandOutcome>) -> Result<CommandOutcome> {
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

/// 待つ相手だけを用意する。
///
/// `wait_with_limit`はpipeもprocess groupも前提にしないため、待機と打ち切りだけを
/// 見るtestは、直接の子を1つ起動して渡す。`sleep`は書き込み直後のfileではないため、
/// `ETXTBSY`でやり直す必要もない。
fn sleeping_child(seconds: &str) -> Checked<std::process::Child> {
    std::process::Command::new("sleep")
        .arg(seconds)
        .stdin(std::process::Stdio::null())
        .spawn()
        .required_because("a child to wait for")
}

/// 子processを`Child`の外で回収し、待てない状態を作る。
///
/// 他のlibraryやsignal handlerが先にwaitした場合と同じで、以後この子は待てない。
fn reaped_outside_the_handle(child: &std::process::Child) -> Checked {
    rustix::process::waitpid(
        Some(rustix::process::Pid::from_child(child)),
        rustix::process::WaitOptions::empty(),
    )
    .required_because("the child is collected outside the handle")?;
    Ok(())
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
    // 転送にかかる時間はrepositoryの大きさが決める。Sandboxを起動する時間では測れない。
    assert!(
        TimeoutClass::RepositoryTransfer.duration() > TimeoutClass::SandboxLifecycle.duration()
    );
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
fn a_relayed_command_sends_both_streams_to_the_external_output_instead_of_a_buffer() -> Checked {
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

    let command = TerminalCommand::relayed(fake.to_str().required()?, &[]);
    let mut output = RecordedOutput::new();
    let outcome = retrying(|| run_with_terminal(&command, &mut output))
        .required_because("the fake tool runs")?;

    assert_eq!(
        fs::read_to_string(&record).required()?,
        "ran",
        "the command still runs"
    );
    let relayed = output.text();
    assert!(
        relayed.contains("progress") && relayed.contains("warning"),
        "both streams reach the terminal through the renderer: {relayed:?}"
    );
    assert_eq!(
        output.finished, 1,
        "the end of the external output is announced once"
    );
    assert_eq!(
        output.handed_over, 0,
        "the terminal itself was not handed over"
    );
    assert!(
        outcome.stdout.is_empty() && outcome.stderr.is_empty(),
        "relayed output belongs to the terminal, not to a buffer"
    );
    assert!(!outcome.stderr_lossy);
    Ok(())
}

#[test]
fn a_relayed_command_that_says_nothing_does_not_announce_any_external_output() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let fake = fake_executable(dir.path(), "fake-tool", "exit 0")?;

    let command = TerminalCommand::relayed(fake.to_str().required()?, &[]);
    let mut output = RecordedOutput::new();
    retrying(|| run_with_terminal(&command, &mut output)).required_because("the fake tool runs")?;

    // 空のbyte列は届くことがある。境界を置くかどうかは描画側が中身で決める。
    assert!(output.relayed.is_empty(), "nothing was written");
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

    let command = TerminalCommand::handed_over(fake.to_str().required()?, &[]);
    // 終える時期を決めるのは利用者であり、sbxmは待ち切る。
    assert_eq!(command.spec().timeout.duration(), None);
    let mut output = RecordedOutput::new();
    let outcome = retrying(|| run_with_terminal(&command, &mut output))
        .required_because("the fake tool runs")?;

    assert_eq!(fs::read_to_string(&record).required()?, "ran");
    assert!(outcome.success());
    assert_eq!(
        (output.handed_over, output.finished),
        (1, 1),
        "the terminal is announced before it is given away and after it comes back"
    );
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
fn an_escaped_descendant_cannot_hold_capture_open_after_timeout() -> Checked {
    // macOS does not ship `setsid` as a command, while Linux does. The process-group behavior is
    // covered where the helper exists; the implementation itself is compiled for both platforms.
    if !exists_on_path("setsid") {
        return Ok(());
    }

    let dir = tempfile::tempdir().required()?;
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        // The detached sleep keeps the pipe open after the original group is killed.
        "setsid sh -c 'sleep 2' &\nsleep 30",
    )?;

    let spec = CommandSpec::probe(fake.to_str().required()?, &[]);
    let started = Instant::now();
    let error = retrying(|| run_with_limit(&spec, Duration::from_millis(200)))
        .refused_because("the command must be terminated")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "the reader must not wait for a process outside the command group"
    );
    Ok(())
}

#[test]
fn a_child_that_outlives_its_limit_is_ended_before_the_timeout_is_reported() -> Checked {
    // 端末へ出すcommandはprocess groupを分けないため、打ち切りは直接の子だけに届く。
    let relayed = TerminalCommand::relayed("sleep", &["30"]);
    let spec = relayed.spec();
    let mut child = sleeping_child("30")?;
    let pid = rustix::process::Pid::from_child(&child);

    let started = Instant::now();
    let error = wait_with_limit(&mut child, spec, Some(Duration::from_millis(200)))
        .refused_because("the limit must end the wait")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the wait must not outlive the limit it was given"
    );
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?;
    assert!(
        diagnostic
            .description
            .args
            .contains(&("program", "sleep".to_string())),
        "the report names the command that was cut off: {:?}",
        diagnostic.description
    );
    // 報告より先に終わらせるため、返った時点でzombieも実行中のprocessも残らない。
    assert_eq!(
        rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG).err(),
        Some(rustix::io::Errno::CHILD),
        "the child must already be collected when the timeout is reported"
    );
    Ok(())
}

#[test]
fn a_child_that_cannot_be_waited_for_is_ended_and_reported_with_what_the_os_said() -> Checked {
    // 対話commandに上限は無い。待てなくなったことだけが、待機を終える理由になる。
    let handed_over = TerminalCommand::handed_over("sleep", &[]);
    let spec = handed_over.spec();
    let mut child = sleeping_child("0")?;
    reaped_outside_the_handle(&child)?;

    let error = wait_with_limit(&mut child, spec, None)
        .refused_because("a child that cannot be waited for must not be waited for forever")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    assert_eq!(
        labels(&error)?,
        vec!["diagnostic-command-label", "diagnostic-cause-label"],
        "the invocation and the reason the OS gave are both kept"
    );
    Ok(())
}

#[test]
fn a_limited_wait_reports_the_same_failure_when_the_child_can_no_longer_be_observed() -> Checked {
    // 期限付きの待機でも、待てなくなった相手は期限まで数え続けずに報告する。
    let relayed = TerminalCommand::relayed("sleep", &[]);
    let spec = relayed.spec();
    let mut child = sleeping_child("0")?;
    reaped_outside_the_handle(&child)?;

    let started = Instant::now();
    let error = wait_with_limit(&mut child, spec, Some(Duration::from_secs(30)))
        .refused_because("a child that cannot be waited for is not waited for")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the failure must be reported without waiting for the limit"
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

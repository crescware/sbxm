use crate::diagnostics::{ErrorId, Result};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
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

/// 背景processの起動を確認してからcaptureのdeadlineを始める。
///
/// 直接の子をspawnしてすぐに200msのtimeoutを始めると、負荷の高いmacOSではshellが
/// 背景processを作る前に打ち切られることがある。このtestは子孫がpipeを握ったまま
/// 直接の子だけが終わることを見たいので、起動完了を待ってからpumpを始める。
fn run_capture_after_ready(
    spec: &CommandSpec,
    ready: &Path,
    limit: Duration,
    started: &mut Option<Instant>,
) -> Result<()> {
    let mut command = configure(spec);
    command.process_group(0);
    let signal = SignalGuard::new().map_err(|error| spawn_failure(spec, &error))?;
    let mut child = spawn(&mut command, spec)?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !ready.exists() {
        terminate_child(&mut child);
        return Err(spawn_failure(
            spec,
            &std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "the capture descendant did not start",
            ),
        ));
    }

    *started = Some(Instant::now());
    let mut output = Vec::new();
    pump_until_exit(
        &mut child,
        spec,
        Some(limit),
        Some(&signal),
        &mut |_, bytes| {
            output.extend_from_slice(bytes);
            Ok(())
        },
    )?;
    Ok(())
}

/// spawnに失敗した試行だけをやり直す。
///
/// 別threadのtestがforkしている最中は、書き込み直後のfileが`ETXTBSY`で起動できない
/// ことがある。実装ではなくtest環境の競合なので、短い間だけ繰り返す。
fn retrying<T>(mut attempt: impl FnMut() -> Result<T>) -> Result<T> {
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
fn the_real_host_uses_the_pty_runner() -> Checked {
    let error = RealHost
        .run_pty_confirmed(&PtyConfirmedCommand::new(
            "/does/not/exist/sbx",
            &[],
            "the sandbox",
            "confirmation",
        ))
        .refused_because("the real host delegates PTY execution")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotFound));
    Ok(())
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
    // 書いたばかりのfake executableは起動に失敗してやり直すことがある。数えるのは
    // 成立した1回の実行だけにする。
    let outcome = retrying(|| {
        output = RecordedOutput::new();
        run_with_terminal(&command, &mut output)
    })
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
fn security_sensitive_runs_touch_no_environment_variable_other_than_the_ssh_agent_socket() {
    // `env` / `env_remove`による明示的な変更は`get_envs()`に現れ、
    // `env_clear`を呼んだ場合もこのassertionの結果が変わる。
    // `DOCKER_SANDBOXES_ROOT_SIZE`のような他の変数は、実際のprocess environmentを
    // 動かさずとも、この一覧が`SSH_AUTH_SOCK`の除去だけであることで素通りすると示せる。
    let spec = CommandSpec::probe("true", &[]).env(EnvPolicy::InheritWithoutSshAgent);
    let command = configure(&spec);
    let envs: Vec<(&std::ffi::OsStr, Option<&std::ffi::OsStr>)> = command.get_envs().collect();
    assert_eq!(envs, [(std::ffi::OsStr::new("SSH_AUTH_SOCK"), None)]);
}

#[test]
fn git_in_a_host_repository_forgets_where_the_caller_pointed_git() {
    // gitのhookやaliasから起動されると、`GIT_DIR`などが利用者のrepositoryと別の場所を
    // 指している。hostのrepositoryで走らせるgitは、それを引き継がない。
    let spec = CommandSpec::probe("git", &[])
        .env(EnvPolicy::HostRepository)
        .working_dir(Path::new("/work/example-repo"));
    let command = configure(&spec);
    let envs: Vec<(&std::ffi::OsStr, Option<&std::ffi::OsStr>)> = command.get_envs().collect();
    let removed = |name: &str| envs.contains(&(std::ffi::OsStr::new(name), None));
    for name in [
        "SSH_AUTH_SOCK",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        // Sandboxへつなぐsshは、sbxmが`core.sshCommand`で決める。
        "GIT_SSH",
        "GIT_SSH_COMMAND",
        "GIT_SSH_VARIANT",
    ] {
        assert!(removed(name), "{name}: {envs:?}");
    }
    assert!(envs.contains(&(
        std::ffi::OsStr::new("GIT_CEILING_DIRECTORIES"),
        Some(std::ffi::OsStr::new("/work"))
    )));
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
fn a_descendant_outliving_the_direct_child_cannot_hold_capture_open_after_timeout() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let ready = dir.path().join("ready");
    let survivor = dir.path().join("survivor");
    let fake = fake_executable(
        dir.path(),
        "fake-tool",
        &format!(
            // 背景のprocessは直接の子が終わったあとにmarkerを残す。stdoutのpipeも握ったままに
            // するため、readerがEOFを待つ実装へ戻るとこのtestもtimeoutする。
            r#"(printf 'ready' > "{}"; sleep 1; printf 'alive' > "{}") &
sleep 30"#,
            ready.display(),
            survivor.display()
        ),
    )?;

    let spec = CommandSpec::probe(fake.to_str().required()?, &[]);
    let mut started = None;
    let error = retrying(|| {
        run_capture_after_ready(&spec, &ready, Duration::from_millis(200), &mut started)
    })
    .refused_because("the command must be terminated")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(
        started
            .required_because("capture must start after the descendant is ready")?
            .elapsed()
            < Duration::from_millis(1500),
        "the reader must not wait for a descendant that outlives the direct child"
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    while !survivor.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read_to_string(&survivor)
            .required_because("the descendant outlives its direct child")?,
        "alive",
        "timeout must not terminate the descendant"
    );
    Ok(())
}

#[test]
fn a_child_that_outlives_its_limit_is_ended_before_the_timeout_is_reported() -> Checked {
    // 打ち切りが届くのは直接の子だけである。
    let relayed = TerminalCommand::relayed("sleep", &["30"]);
    let spec = relayed.spec();
    let mut child = sleeping_child("30")?;
    let pid = rustix::process::Pid::from_child(&child);

    let started = Instant::now();
    let error = wait_with_limit(&mut child, spec, Some(Duration::from_millis(200)), None)
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

    let error = wait_with_limit(&mut child, spec, None, None)
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
    let error = wait_with_limit(&mut child, spec, Some(Duration::from_secs(30)), None)
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
fn a_missing_working_directory_is_not_mistaken_for_a_missing_program() -> Checked {
    // OSはどちらも`NotFound`で答える。無いのはprogramではなくdirectoryだと名指しする。
    let dir = tempfile::tempdir().required()?;
    let spec = CommandSpec::probe("true", &[]).working_dir(&dir.path().join("gone"));
    let error = run(&spec).refused_because("the directory is gone")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandDirectoryMissing)
    );
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

#[test]
fn input_reaches_the_child_and_ends_with_an_eof() -> Checked {
    let spec = CommandSpec::capture("sh", &["-c", "cat"]).with_input(b"declared = true\n".to_vec());
    let outcome = run_fake(&spec).required()?;
    assert!(outcome.status.success());
    assert_eq!(outcome.stdout, b"declared = true\n");
    Ok(())
}

#[test]
fn input_larger_than_a_pipe_is_written_while_the_output_is_read() -> Checked {
    // 子は読んだ分だけ書き返す。書き込みと読み取りのどちらかが待つと、互いに止まる。
    let input: Vec<u8> = (0..=u8::MAX).cycle().take(1024 * 1024).collect();
    let spec = CommandSpec::capture("sh", &["-c", "cat"]).with_input(input.clone());
    let outcome = run_fake(&spec).required()?;
    assert!(outcome.status.success());
    assert_eq!(outcome.stdout.len(), input.len());
    assert!(outcome.stdout == input, "the bytes arrive unchanged");
    Ok(())
}

#[test]
fn a_child_that_never_reads_its_input_still_ends_with_its_own_status() -> Checked {
    let input = vec![0_u8; 1024 * 1024];
    let spec = CommandSpec::capture("sh", &["-c", "exit 3"]).with_input(input);
    let outcome = run_fake(&spec).required()?;
    assert_eq!(outcome.status.code(), Some(3));
    Ok(())
}

#[test]
fn the_input_never_reaches_a_debug_representation() {
    // 宣言fileの中身を運ぶ。表示や記録へ出る経路を作らない。
    let spec = CommandSpec::capture("sh", &["-c", "cat"]).with_input(b"token=secret".to_vec());
    let shown = format!("{spec:?}");
    assert!(!shown.contains("secret"), "{shown}");
    assert!(shown.contains("12 bytes"), "{shown}");
    assert_eq!(spec.input(), Some(b"token=secret".as_slice()));
}

// --- stdoutを流して受け取る実行 ---

#[test]
fn a_streamed_run_hands_stdout_to_the_sink_and_keeps_stderr() -> Checked {
    let spec = CommandSpec::capture("sh", &["-c", "printf received; printf noted >&2"]);
    let mut sink = Vec::new();
    let outcome = retrying(|| run_streaming(&spec, &mut sink, 1024)).required()?;
    assert!(outcome.status.success());
    assert_eq!(sink, b"received");
    assert!(outcome.stdout.is_empty(), "stdout is not kept twice");
    assert_eq!(outcome.stderr, b"noted");
    Ok(())
}

#[test]
fn a_streamed_run_carries_more_than_a_pipe_holds() -> Checked {
    let spec = CommandSpec::capture("head", &["-c", "2097152", "/dev/zero"]);
    let mut sink = Vec::new();
    let outcome = retrying(|| run_streaming(&spec, &mut sink, 4 * 1024 * 1024)).required()?;
    assert!(outcome.status.success());
    assert_eq!(sink.len(), 2 * 1024 * 1024);
    Ok(())
}

#[test]
fn output_beyond_the_limit_ends_the_child_and_is_refused() -> Checked {
    // 終わらない出力でも、上限を超えた時点で子を終わらせる。
    let spec = CommandSpec::capture("yes", &[]);
    let mut sink = Vec::new();
    let started = Instant::now();
    let error = retrying(|| run_streaming(&spec, &mut sink, 4096))
        .refused_because("the output never ends")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputTooLarge)
    );
    assert!(sink.len() <= 4096, "nothing beyond the limit is kept");
    assert!(started.elapsed() < Duration::from_secs(5));
    Ok(())
}

/// 受け取れないと答える書き込み先。
struct RefusingSink;

impl std::io::Write for RefusingSink {
    fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("no space left for the output"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_sink_that_cannot_take_the_output_refuses_the_run() -> Checked {
    let spec = CommandSpec::capture("sh", &["-c", "printf received"]);
    let error = retrying(|| run_streaming(&spec, &mut RefusingSink, 1024))
        .refused_because("the output has nowhere to go")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnstored)
    );
    Ok(())
}

/// captureしたstdoutを決め打ちで返すhost。既定の`run_streaming`を確かめる。
struct AnsweringHost(Vec<u8>);

impl HostEnvironment for AnsweringHost {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        Ok(crate::testing::command::outcome(
            spec,
            0,
            &String::from_utf8_lossy(&self.0),
        ))
    }
}

#[test]
fn a_host_without_streaming_hands_over_what_it_captured() -> Checked {
    let spec = CommandSpec::capture("sbx", &["exec"]);
    let mut sink = Vec::new();
    let outcome = AnsweringHost(b"bundle".to_vec())
        .run_streaming(&spec, &mut sink, 1024)
        .required()?;
    assert_eq!(sink, b"bundle");
    assert!(outcome.stdout.is_empty());

    let error = AnsweringHost(b"bundle".to_vec())
        .run_streaming(&spec, &mut Vec::new(), 3)
        .refused_because("the captured output is larger than allowed")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputTooLarge)
    );
    let error = AnsweringHost(b"bundle".to_vec())
        .run_streaming(&spec, &mut RefusingSink, 1024)
        .refused_because("the output has nowhere to go")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnstored)
    );
    Ok(())
}

#[test]
fn a_host_without_ticking_runs_the_command_and_leaves_the_tick_alone() -> Checked {
    // 既定は途中の手続きを呼ばない。実物と違う順序で呼べば、testが実物の経路を見誤る。
    let command = TerminalCommand::handed_over("ssh", &["example.sbx"]);
    let mut output = RecordedOutput::new();
    let mut ticks = 0;
    let mut tick = || ticks += 1;
    let outcome = AnsweringHost(b"session".to_vec())
        .run_with_terminal_ticking(&command, &mut output, Duration::from_millis(1), &mut tick)
        .required()?;
    assert!(outcome.success());
    assert_eq!(ticks, 0);
    assert_eq!(output.finished, 1);
    Ok(())
}

/// 受け取れるが、書き終えられない書き込み先。
struct UnflushableSink(Vec<u8>);

impl std::io::Write for UnflushableSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("the output could not be finished"))
    }
}

#[test]
fn a_sink_that_cannot_finish_the_output_refuses_the_run() -> Checked {
    let spec = CommandSpec::capture("sh", &["-c", "printf received"]);
    let error = retrying(|| run_streaming(&spec, &mut UnflushableSink(Vec::new()), 1024))
        .refused_because("the output was not finished")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnstored)
    );
    Ok(())
}

#[test]
fn the_real_host_streams_stdout_itself() -> Checked {
    let spec = CommandSpec::capture("sh", &["-c", "printf received"]);
    let mut sink = Vec::new();
    retrying(|| RealHost.run_streaming(&spec, &mut sink, 1024)).required()?;
    assert_eq!(sink, b"received");
    Ok(())
}

#[test]
fn a_handed_over_command_lets_the_caller_work_while_it_runs() -> Checked {
    let command = TerminalCommand::handed_over("sh", &["-c", "sleep 0.3"]);
    let mut ticks = 0;

    let outcome = RealHost
        .run_with_terminal_ticking(
            &command,
            &mut RecordedOutput::new(),
            Duration::from_millis(50),
            &mut || ticks += 1,
        )
        .required()?;

    assert!(outcome.success());
    assert!(
        ticks >= 2,
        "the caller worked while the command ran: {ticks}"
    );
    Ok(())
}

#[test]
fn a_command_that_keeps_its_output_is_not_interrupted_to_tick() -> Checked {
    let command = TerminalCommand::relayed("sh", &["-c", "sleep 0.1"]);
    let mut ticks = 0;

    let outcome = RealHost
        .run_with_terminal_ticking(
            &command,
            &mut RecordedOutput::new(),
            Duration::from_millis(10),
            &mut || ticks += 1,
        )
        .required()?;

    assert!(outcome.success());
    assert_eq!(ticks, 0);
    Ok(())
}

#[test]
fn only_the_first_part_of_a_flood_of_diagnostics_is_kept() -> Checked {
    // 流す実行が読む相手は信用しない。stderrは診断に使う分だけ溜め、残りは読んで捨てる。
    let spec = CommandSpec::capture("sh", &["-c", "head -c 300000 /dev/zero >&2; printf done"]);
    let mut sink = Vec::new();
    let outcome = retrying(|| RealHost.run_streaming(&spec, &mut sink, 1024)).required()?;
    assert_eq!(sink, b"done");
    assert_eq!(outcome.stderr.len(), 64 * 1024);
    Ok(())
}

#[test]
fn a_large_input_reaches_a_child_that_writes_nothing_without_waiting_on_polls() -> Checked {
    // 子が出力を書かないあいだも、読んだ分だけすぐに書き足す。出力を待つ間隔ごとに
    // 書き足していた頃は、8 MiBに0.7秒ほどかかった。今は数msで終わる。
    let input = vec![b'x'; 8 * 1024 * 1024];
    let spec = CommandSpec::capture("sh", &["-c", "cat > /dev/null"]).with_input(input);
    let started = Instant::now();

    let outcome = RealHost.run(&spec).required()?;

    assert!(outcome.success());
    assert!(
        started.elapsed() < Duration::from_millis(300),
        "{:?}",
        started.elapsed()
    );
    Ok(())
}

#[test]
fn a_wait_that_ticks_still_ends_a_child_that_outlives_its_limit() -> Checked {
    // 待つあいだ手続きを呼ぶ経路でも、上限は守る。
    let handed_over = TerminalCommand::handed_over("sleep", &["30"]);
    let spec = handed_over.spec();
    let mut child = sleeping_child("30")?;
    let mut ticks = 0;

    let started = Instant::now();
    let error = wait_with_limit(
        &mut child,
        spec,
        Some(Duration::from_millis(200)),
        Some((Duration::from_millis(20), &mut || ticks += 1)),
    )
    .refused_because("the limit must end the wait")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(ticks >= 1, "the caller worked while waiting: {ticks}");
    Ok(())
}

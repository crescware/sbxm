use std::io::{self, ErrorKind};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::testing::command::{
    End, Event, ReadStep, ScriptedOs, ScriptedPipe, ScriptedWriter, Step,
};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;
use std::fs;

/// probeとして走らせる、既定のcommand。
fn spec() -> CommandSpec {
    CommandSpec::probe("fake-tool", &[])
}

/// `code`で自ら終わった子の終了status。
fn exited(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

/// 別の場所で引き取られた子を尋ねたときの失敗。
fn no_child() -> io::Error {
    io::Error::from_raw_os_error(rustix::io::Errno::CHILD.raw_os_error())
}

/// 子を終わらせ、終了statusを引き取ったか。
fn ended(events: &[Event]) -> bool {
    events
        .windows(2)
        .any(|pair| pair == [Event::Ended, Event::WaitedExit])
}

/// 診断が持つ事実の項目名。
fn labels(error: &Error) -> Checked<Vec<String>> {
    Ok(error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .map(|fact| fact.label().id.to_string())
        .collect())
}

/// `spec`を起動したという記録。
fn started(spec: &CommandSpec) -> Event {
    Event::Started {
        program: spec.program.clone(),
        args: spec.args.clone(),
        directory: spec.working_dir.clone(),
    }
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
    let os = ScriptedOs::default().exits([Ok(Some(exited(0)))]);
    let spec = CommandSpec::capture("fake-tool", &[]).working_dir(&workspace);

    run_inner(&os, os.clock(), &spec, None).required_because("the fake tool runs")?;

    assert!(
        os.events().contains(&started(&spec)),
        "the working directory reaches the started command: {:?}",
        os.events()
    );
    Ok(())
}

#[test]
fn a_relayed_command_sends_both_streams_to_the_external_output_instead_of_a_buffer() -> Checked {
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::new([ReadStep::Bytes(b"progress")]))
        .stderr(ScriptedPipe::new([ReadStep::Bytes(b"warning")]))
        .exits([Ok(Some(exited(0)))]);
    let command = TerminalCommand::relayed("fake-tool", &[]);
    let mut output = RecordedOutput::new();

    let outcome = run_terminal_inner(&os, os.clock(), &command, &mut output, None)
        .required_because("the fake tool runs")?;

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
    assert!(
        !os.events().contains(&Event::OwnGroup),
        "the relay stays in sbxm's own process group"
    );
    Ok(())
}

#[test]
fn a_relayed_command_that_says_nothing_does_not_announce_any_external_output() -> Checked {
    let os = ScriptedOs::default().exits([Ok(Some(exited(0)))]);
    let command = TerminalCommand::relayed("fake-tool", &[]);
    let mut output = RecordedOutput::new();

    run_terminal_inner(&os, os.clock(), &command, &mut output, None)
        .required_because("the fake tool runs")?;

    // 空のbyte列は届くことがある。境界を置くかどうかは描画側が中身で決める。
    assert!(output.relayed.is_empty(), "nothing was written");
    Ok(())
}

#[test]
fn an_interactive_command_is_handed_the_terminal_and_waited_for_without_a_limit() -> Checked {
    let os = ScriptedOs::default();
    let command = TerminalCommand::handed_over("fake-tool", &[]);
    // 終える時期を決めるのは利用者であり、sbxmは待ち切る。
    assert_eq!(command.spec().timeout.duration(), None);
    let mut output = RecordedOutput::new();

    let outcome = run_terminal_inner(&os, os.clock(), &command, &mut output, None)
        .required_because("the fake tool runs")?;

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
    assert_eq!(
        os.events(),
        [started(command.spec()), Event::WaitedExit],
        "without a limit or a tick, the wait never asks in between"
    );
    Ok(())
}

#[test]
fn a_failure_keeps_the_invocation_that_produced_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let workspace = dir.path().join("workspace");
    fs::create_dir(&workspace).required()?;
    let os = ScriptedOs::default().exits([Ok(Some(exited(2)))]);
    let spec = CommandSpec::capture("fake-tool", &["clone", "--bare"]).working_dir(&workspace);

    let failure = run_inner(&os, os.clock(), &spec, None)
        .required_because("runs")?
        .failure();

    assert_eq!(failure.safe_args, vec!["clone", "--bare"]);
    assert_eq!(failure.working_dir.as_deref(), Some(workspace.as_path()));
    assert!(failure.exit_status.contains('2'));
    Ok(())
}

#[test]
fn every_argument_reaches_the_program_in_order() -> Checked {
    let os = ScriptedOs::default().exits([Ok(Some(exited(0)))]);
    let spec = CommandSpec::probe("fake-tool", &["ls", "--json"]);

    run_inner(&os, os.clock(), &spec, None).required_because("the fake tool runs")?;

    assert!(os.events().contains(&started(&spec)), "{:?}", os.events());
    Ok(())
}

#[test]
fn arguments_are_passed_without_a_shell() -> Checked {
    // shellを介さないため、metacharacterはそのまま1個のargumentとして届く。
    let dangerous = "; rm -rf / #$(whoami)";
    let os = ScriptedOs::default().exits([Ok(Some(exited(0)))]);
    let spec = CommandSpec::probe("fake-tool", &[dangerous]);

    run_inner(&os, os.clock(), &spec, None).required_because("the fake tool runs")?;

    assert!(os.events().contains(&started(&spec)), "{:?}", os.events());
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
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::new([ReadStep::Bytes(b"to stdout")]))
        .stderr(ScriptedPipe::new([ReadStep::Bytes(b"to stderr")]))
        .exits([Ok(Some(exited(3)))]);

    let outcome = run_inner(&os, os.clock(), &spec(), None).required_because("runs")?;

    assert_eq!(outcome.stdout_text(), "to stdout");
    assert_eq!(outcome.failure().stderr_text(), "to stderr");
    assert!(!outcome.success());
    assert_eq!(outcome.status.code(), Some(3));
    Ok(())
}

#[test]
fn invalid_utf8_output_is_kept_as_bytes_and_reported_as_lossy() -> Checked {
    let os = ScriptedOs::default()
        .stderr(ScriptedPipe::new([ReadStep::Bytes(b"\xff\xfe")]))
        .exits([Ok(Some(exited(0)))]);

    let outcome = run_inner(&os, os.clock(), &spec(), None).required_because("runs")?;

    assert_eq!(outcome.stderr, vec![0xff, 0xfe]);
    assert!(
        outcome.stderr_lossy,
        "a lossy conversion must be reported as such"
    );
    Ok(())
}

#[test]
fn a_command_that_exceeds_its_timeout_is_terminated() -> Checked {
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::held_open([]))
        .stderr(ScriptedPipe::held_open([]));

    let error = run_inner(&os, os.clock(), &spec(), Some(Duration::from_millis(200)))
        .refused_because("the command must be terminated")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert_eq!(
        os.events()
            .iter()
            .filter(|event| matches!(event, Event::Watched(_)))
            .count(),
        10,
        "a 200ms limit is reached after 10 waits of 20ms each"
    );
    assert!(ended(&os.events()), "{:?}", os.events());
    Ok(())
}

#[test]
fn a_descendant_outliving_the_direct_child_cannot_hold_capture_open_after_timeout() -> Checked {
    // 背景の子孫がpipeを握ったままでも、直接の子が終わればcaptureも終わる。EOFを待てば、
    // このtestは終わらない。
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::held_open([]))
        .stderr(ScriptedPipe::held_open([]))
        .exits([Ok(None), Ok(Some(exited(0)))]);

    let outcome = run_inner(&os, os.clock(), &spec(), None)
        .required_because("the direct child ends the capture")?;

    assert!(outcome.success());
    assert!(os.events().contains(&Event::Closed(End::Stdout)));
    assert!(os.events().contains(&Event::Closed(End::Stderr)));
    Ok(())
}

#[test]
fn a_child_that_outlives_its_limit_is_ended_before_the_timeout_is_reported() -> Checked {
    // 打ち切りが届くのは直接の子だけである。
    let relayed = TerminalCommand::relayed("fake-tool", &["30"]);
    let spec = relayed.spec();
    let os = ScriptedOs::default();

    let error = wait_with_limit(
        &os,
        os.clock(),
        &mut (),
        spec,
        Some(Duration::from_millis(200)),
        None,
    )
    .refused_because("the limit must end the wait")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?;
    assert!(
        diagnostic
            .description
            .args
            .contains(&("program", "fake-tool".to_string())),
        "the report names the command that was cut off: {:?}",
        diagnostic.description
    );
    assert!(
        ended(&os.events()),
        "the child must already be collected when the timeout is reported"
    );
    Ok(())
}

#[test]
fn a_child_that_cannot_be_waited_for_is_ended_and_reported_with_what_the_os_said() -> Checked {
    // 対話commandに上限は無い。待てなくなったことだけが、待機を終える理由になる。
    let handed_over = TerminalCommand::handed_over("fake-tool", &[]);
    let spec = handed_over.spec();
    let os = ScriptedOs::default().waits([Err(no_child())]);

    let error = wait_with_limit(&os, os.clock(), &mut (), spec, None, None)
        .refused_because("a child that cannot be waited for must not be waited for forever")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    assert_eq!(
        labels(&error)?,
        vec!["diagnostic-command-label", "diagnostic-cause-label"],
        "the invocation and the reason the OS gave are both kept"
    );
    assert!(ended(&os.events()), "{:?}", os.events());
    Ok(())
}

#[test]
fn a_limited_wait_reports_the_same_failure_when_the_child_can_no_longer_be_observed() -> Checked {
    // 期限付きの待機でも、待てなくなった相手は期限まで数え続けずに報告する。
    let relayed = TerminalCommand::relayed("fake-tool", &[]);
    let spec = relayed.spec();
    let os = ScriptedOs::default().exits([Err(no_child())]);

    let error = wait_with_limit(
        &os,
        os.clock(),
        &mut (),
        spec,
        Some(Duration::from_secs(30)),
        None,
    )
    .refused_because("a child that cannot be waited for is not waited for")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandSpawnFailed));
    assert!(
        os.clock().slept.borrow().is_empty(),
        "the failure must be reported without waiting for the limit"
    );
    Ok(())
}

#[test]
fn a_missing_program_is_distinguished_from_other_spawn_failures() -> Checked {
    let os = ScriptedOs::default().failing(Step::Start, io::Error::from(ErrorKind::NotFound));
    let error =
        run_inner(&os, os.clock(), &spec(), None).refused_because("missing programs fail")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandNotFound));
    Ok(())
}

#[test]
fn a_missing_working_directory_is_not_mistaken_for_a_missing_program() -> Checked {
    // OSはどちらも`NotFound`で答える。無いのはprogramではなくdirectoryだと名指しする。
    let dir = tempfile::tempdir().required()?;
    let os = ScriptedOs::default().failing(Step::Start, io::Error::from(ErrorKind::NotFound));
    let spec = CommandSpec::probe("fake-tool", &[]).working_dir(&dir.path().join("gone"));

    let error = run_inner(&os, os.clock(), &spec, None).refused_because("the directory is gone")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandDirectoryMissing)
    );
    Ok(())
}

#[test]
fn a_spawn_failure_that_is_not_a_missing_program_keeps_what_was_observed() -> Checked {
    // 起動できない理由が「相手が居ない」でなければ、相手が居ないとは言わない。
    let os =
        ScriptedOs::default().failing(Step::Start, io::Error::from(ErrorKind::PermissionDenied));

    let error = run_inner(&os, os.clock(), &spec(), None)
        .refused_because("a directory cannot be started")?;

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
    let os = ScriptedOs::default()
        .stderr(ScriptedPipe::new([ReadStep::Bytes(b"boom")]))
        .exits([Ok(Some(exited(42)))]);

    let outcome = run_inner(&os, os.clock(), &spec(), None).required_because("runs")?;
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
    // 実行可能fileは実行時に書かない。置いてある`/bin/sh`を別の名前で指す。
    std::os::unix::fs::symlink("/bin/sh", dir.path().join("sbxm-fake-on-path"))
        .required_because("link the fake executable")?;

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
    let writer = ScriptedWriter::accepting_all();
    let written = writer.written();
    let os = ScriptedOs::default()
        .stdin(writer)
        .exits([Ok(Some(exited(0)))]);
    let spec = spec().with_input(b"declared = true\n".to_vec());

    let outcome = run_inner(&os, os.clock(), &spec, None).required()?;

    assert!(outcome.success());
    assert_eq!(written.borrow().as_slice(), b"declared = true\n");
    assert!(os.events().contains(&Event::Closed(End::Stdin)));
    Ok(())
}

#[test]
fn input_larger_than_a_pipe_is_written_while_the_output_is_read() -> Checked {
    // 子は読んだ分だけ書き返す。台本は書けた量だけ進め、要らない容量の仮定は置かない。
    const CHUNK: usize = 4096;
    let input: Vec<u8> = (0..=u8::MAX).cycle().take(4 * CHUNK).collect();
    let mut answers = Vec::new();
    let mut stdout_steps = Vec::new();
    for chunk in input.chunks(CHUNK) {
        answers.push(Ok(CHUNK));
        answers.push(Err(ErrorKind::WouldBlock.into()));
        stdout_steps.push(ReadStep::Owned(chunk.to_vec()));
    }
    let writer = ScriptedWriter::answering(answers);
    let written = writer.written();
    let os = ScriptedOs::default()
        .stdin(writer)
        .stdout(ScriptedPipe::new(stdout_steps))
        .exits([
            Ok(None),
            Ok(None),
            Ok(None),
            Ok(None),
            Ok(None),
            Ok(Some(exited(0))),
        ]);
    let spec = spec().with_input(input.clone());

    let outcome = run_inner(&os, os.clock(), &spec, None).required()?;

    assert!(outcome.success());
    assert_eq!(outcome.stdout, input);
    assert_eq!(written.borrow().as_slice(), input.as_slice());
    Ok(())
}

#[test]
fn a_child_that_never_reads_its_input_still_ends_with_its_own_status() -> Checked {
    let writer = ScriptedWriter::answering([Err(ErrorKind::BrokenPipe.into())]);
    let os = ScriptedOs::default()
        .stdin(writer)
        .exits([Ok(Some(exited(3)))]);
    let spec = spec().with_input(vec![0_u8; 1024 * 1024]);

    let outcome = run_inner(&os, os.clock(), &spec, None).required()?;

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
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::new([ReadStep::Bytes(b"received")]))
        .stderr(ScriptedPipe::new([ReadStep::Bytes(b"noted")]))
        .exits([Ok(Some(exited(0)))]);
    let mut sink = Vec::new();

    let outcome = run_streaming(&os, os.clock(), &spec(), &mut sink, 1024).required()?;

    assert!(outcome.status.success());
    assert_eq!(sink, b"received");
    assert!(outcome.stdout.is_empty(), "stdout is not kept twice");
    assert_eq!(outcome.stderr, b"noted");
    Ok(())
}

#[test]
fn a_streamed_run_carries_more_than_a_pipe_holds() -> Checked {
    let steps = (0..64).map(|_| ReadStep::Owned(vec![b'x'; 32 * 1024]));
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::new(steps))
        .exits([Ok(Some(exited(0)))]);
    let mut sink = Vec::new();

    let outcome = run_streaming(&os, os.clock(), &spec(), &mut sink, 4 * 1024 * 1024).required()?;

    assert!(outcome.status.success());
    assert_eq!(sink.len(), 2 * 1024 * 1024);
    Ok(())
}

#[test]
fn output_beyond_the_limit_ends_the_child_and_is_refused() -> Checked {
    // 終わらない出力でも、上限を超えた時点で子を終わらせる。
    let steps = (0..3).map(|_| ReadStep::Owned(vec![b'y'; 4096]));
    let os = ScriptedOs::default().stdout(ScriptedPipe::held_open(steps));
    let mut sink = Vec::new();

    let error = run_streaming(&os, os.clock(), &spec(), &mut sink, 4096)
        .refused_because("the output never ends")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputTooLarge)
    );
    assert!(sink.len() <= 4096, "nothing beyond the limit is kept");
    assert!(ended(&os.events()), "{:?}", os.events());
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
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::new([ReadStep::Bytes(b"received")]))
        .exits([Ok(Some(exited(0)))]);

    let error = run_streaming(&os, os.clock(), &spec(), &mut RefusingSink, 1024)
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
    let os = ScriptedOs::default()
        .stdout(ScriptedPipe::new([ReadStep::Bytes(b"received")]))
        .exits([Ok(Some(exited(0)))]);

    let error = run_streaming(
        &os,
        os.clock(),
        &spec(),
        &mut UnflushableSink(Vec::new()),
        1024,
    )
    .refused_because("the output was not finished")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ExternalCommandOutputUnstored)
    );
    Ok(())
}

#[test]
fn a_handed_over_command_lets_the_caller_work_while_it_runs() -> Checked {
    let os = ScriptedOs::default().exits_at(Duration::from_millis(300), 0);
    let mut ticks = 0;
    let mut tick = || ticks += 1;

    let status = wait_with_limit(
        &os,
        os.clock(),
        &mut (),
        &spec(),
        None,
        Some((Duration::from_millis(50), &mut tick)),
    )
    .required_because("the command ends on its own")?;

    assert!(status.success());
    assert_eq!(ticks, 4, "ticks land at 60, 120, 180 and 240ms");
    Ok(())
}

#[test]
fn a_tick_that_takes_time_counts_the_next_interval_from_after_it_returns() -> Checked {
    // `tick`が時計を進めても、次の間隔はその後の時刻から数える。先の時刻から数えていれば、
    // このtestは5回のtickを見る。
    let os = ScriptedOs::default().exits_at(Duration::from_millis(300), 0);
    let mut ticks = 0;
    let mut tick = || {
        ticks += 1;
        os.clock().advance(Duration::from_millis(30));
    };

    wait_with_limit(
        &os,
        os.clock(),
        &mut (),
        &spec(),
        None,
        Some((Duration::from_millis(50), &mut tick)),
    )
    .required_because("the command ends on its own")?;

    assert_eq!(ticks, 3);
    Ok(())
}

#[test]
fn a_command_that_keeps_its_output_is_not_interrupted_to_tick() -> Checked {
    let os = ScriptedOs::default().exits([Ok(Some(exited(0)))]);
    let command = TerminalCommand::relayed("fake-tool", &[]);
    let mut ticks = 0;
    let mut tick = || ticks += 1;
    let mut output = RecordedOutput::new();

    let outcome = run_terminal_inner(
        &os,
        os.clock(),
        &command,
        &mut output,
        Some((Duration::from_millis(10), &mut tick)),
    )
    .required_because("the fake tool runs")?;

    assert!(outcome.success());
    assert_eq!(ticks, 0);
    assert!(os.clock().slept.borrow().is_empty());
    Ok(())
}

#[test]
fn only_the_first_part_of_a_flood_of_diagnostics_is_kept() -> Checked {
    // 流す実行が読む相手は信用しない。stderrは診断に使う分だけ溜め、残りは読んで捨てる。
    let steps = (0..30).map(|_| ReadStep::Owned(vec![0_u8; 10_000]));
    let os = ScriptedOs::default()
        .stderr(ScriptedPipe::new(steps))
        .exits([Ok(Some(exited(0)))]);
    let mut sink = Vec::new();

    let outcome = run_streaming(&os, os.clock(), &spec(), &mut sink, 1024).required()?;

    assert_eq!(outcome.stderr.len(), 64 * 1024);
    Ok(())
}

#[test]
fn a_large_input_reaches_a_child_that_writes_nothing_without_waiting_on_polls() -> Checked {
    // 書き残しがある間はstdinも見張り、書き終えたら外れる。
    let writer = ScriptedWriter::answering([Ok(1), Err(ErrorKind::WouldBlock.into()), Ok(1)]);
    let os = ScriptedOs::default()
        .stdin(writer)
        .stdout(ScriptedPipe::held_open([]))
        .stderr(ScriptedPipe::held_open([]))
        .exits([Ok(None), Ok(None), Ok(Some(exited(0)))]);
    let spec = spec().with_input(b"ab".to_vec());

    run_inner(&os, os.clock(), &spec, None)
        .required_because("a child that never reads still ends")?;

    let watched: Vec<Vec<End>> = os
        .events()
        .into_iter()
        .filter_map(|event| match event {
            Event::Watched(ends) => Some(ends),
            _ => None,
        })
        .collect();
    assert_eq!(
        watched,
        [
            vec![End::Stdout, End::Stderr, End::Stdin],
            vec![End::Stdout, End::Stderr],
            vec![End::Stdout, End::Stderr],
        ]
    );
    Ok(())
}

#[test]
fn a_wait_that_ticks_still_ends_a_child_that_outlives_its_limit() -> Checked {
    // 待つあいだ手続きを呼ぶ経路でも、上限は守る。
    let handed_over = TerminalCommand::handed_over("fake-tool", &["30"]);
    let spec = handed_over.spec();
    let os = ScriptedOs::default();
    let mut ticks = 0;
    let mut tick = || ticks += 1;

    let error = wait_with_limit(
        &os,
        os.clock(),
        &mut (),
        spec,
        Some(Duration::from_millis(200)),
        Some((Duration::from_millis(20), &mut tick)),
    )
    .refused_because("the limit must end the wait")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert_eq!(ticks, 9, "the caller worked while waiting");
    Ok(())
}

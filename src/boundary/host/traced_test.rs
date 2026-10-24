use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::boundary::host::protocol::{inside_check_arguments, split_inside_check};
use crate::design::Trace;
use crate::design::policy::{CharacterSet, StreamPolicy};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::i18n::Locale;
use crate::testing::command::outcome_with_stderr;
use crate::testing::host::AnsweredHost;
use crate::testing::outcome::{Checked, Required};
use crate::testing::recorded_output::RecordedOutput;
use crate::testing::scripted_clock::ScriptedClock;
use crate::testing::shared_buffer::SharedBuffer;
use crate::time::Clock;

use super::{CommandOutcome, CommandSpec, HostEnvironment, TerminalCommand, Traced};

/// 答えるたびに時計を進めるhost。
struct SlowHost<'a> {
    clock: &'a ScriptedClock,
    takes: Duration,
    /// 受けた起動。
    seen: RefCell<Vec<CommandSpec>>,
    /// 結果を返さずに止まるか。
    fails: bool,
    /// まとめた確認への答え。
    inside: Option<String>,
    /// 端末を渡したsessionが続く時間。
    session: Duration,
}

impl<'a> SlowHost<'a> {
    fn new(clock: &'a ScriptedClock, takes: Duration) -> SlowHost<'a> {
        SlowHost {
            clock,
            takes,
            seen: RefCell::new(Vec::new()),
            fails: false,
            inside: None,
            session: Duration::ZERO,
        }
    }
}

impl AnsweredHost for SlowHost<'_> {
    fn has_command(&self, _program: &str) -> bool {
        true
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.seen.borrow_mut().push(spec.clone());
        self.clock.advance(self.takes);
        if self.fails {
            return Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                crate::msg!("test-only-timeout"),
            ));
        }
        if let Some(inside) = &self.inside {
            return Ok(outcome_with_stderr(spec, 0, inside, ""));
        }
        Ok(outcome_with_stderr(spec, 0, "out", "err!"))
    }

    fn session_length(&self, _command: &TerminalCommand, _every: Duration) -> Duration {
        self.session
    }
}

/// traceの行の幅。最も長い`done`の行も、時間の前に空白を残して収まる。
const WIDTH: usize = 130;

fn policy() -> StreamPolicy {
    StreamPolicy {
        color: false,
        characters: CharacterSet::Unicode,
        width: Some(WIDTH),
    }
}

fn clock() -> Rc<ScriptedClock> {
    Rc::new(ScriptedClock::default())
}

/// 12msで答えるhostを包んで`act`を走らせ、traceへ並んだ行を返す。
fn traced(clock: &Rc<ScriptedClock>, act: impl FnOnce(&dyn HostEnvironment)) -> Vec<String> {
    traced_with(clock, &SlowHost::new(clock, Duration::from_millis(12)), act)
}

fn traced_with(
    clock: &Rc<ScriptedClock>,
    host: &SlowHost<'_>,
    act: impl FnOnce(&dyn HostEnvironment),
) -> Vec<String> {
    let buffer = SharedBuffer::new();
    let reader: Rc<dyn Clock> = Rc::<ScriptedClock>::clone(clock);
    let trace = Rc::new(RefCell::new(Trace::new(
        Locale::En,
        policy(),
        buffer.clone(),
        reader,
    )));
    act(&Traced::new(host, trace));
    buffer.lines()
}

fn ends_with_time(line: &str, time: &str) -> bool {
    line.len() == WIDTH && line.ends_with(&format!(" {time}"))
}

/// `commands`を走らせる、まとめた確認の`sbx exec`。
fn inside_check_spec(commands: &[Vec<String>], timed: bool) -> CommandSpec {
    let mut args = vec!["exec".to_string(), "org-repo".to_string(), "--".to_string()];
    args.extend(inside_check_arguments(commands, timed));
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    CommandSpec::capture("sbx", &borrowed)
}

#[test]
fn a_captured_run_is_traced_with_its_command_status_and_sizes() {
    let clock = clock();
    let spec = CommandSpec::probe("fake-tool", &["version", "--format", "{{.Server.Version}}"]);
    let lines = traced(&clock, |host| {
        let outcome = host.run(&spec).ok();
        assert_eq!(outcome.map(|outcome| outcome.stdout), Some(b"out".to_vec()));
    });

    assert_eq!(
        lines[0],
        "run     fake-tool version --format '{{.Server.Version}}'"
    );
    assert!(
        lines[1].starts_with(
            "done    fake-tool version --format '{{.Server.Version}}': exit status: 0, stdout 3B, stderr 4B "
        ),
        "{lines:?}"
    );
    assert!(ends_with_time(&lines[1], "12.000ms"), "{lines:?}");
    assert_eq!(lines.len(), 2);
}

#[test]
fn an_inside_check_is_asked_to_time_its_commands_and_each_one_is_traced() -> Checked {
    let clock = clock();
    let commands = vec![
        vec!["printenv".to_string(), "SSH_AUTH_SOCK".to_string()],
        vec!["ssh-add".to_string(), "-L".to_string()],
    ];
    let spec = inside_check_spec(&commands, false);
    let mut host = SlowHost::new(&clock, Duration::from_millis(1_244));
    host.inside = Some("0 1 0 0 3102000\n1 2 0 0 -\n".to_string());

    let lines = traced_with(&clock, &host, |traced| {
        let outcome = traced.run(&spec).ok();
        // 呼び出し側には、並べた結果がそのまま返る。
        assert_eq!(
            outcome.map(|outcome| outcome.stdout),
            Some(b"0 1 0 0 3102000\n1 2 0 0 -\n".to_vec())
        );
    });

    // 実際に起動したのは、所要時間を計る手順である。
    let seen = host.seen.borrow();
    let (sandbox, timed, split) =
        split_inside_check(&seen[0].args).required_because("the check")?;
    assert_eq!(
        (sandbox.as_str(), timed, split),
        ("org-repo", true, commands.clone())
    );
    assert_eq!(
        lines[0],
        "run     sbx exec org-repo -- sh -c <inside_check.sh> sh -t (2 commands)"
    );
    assert!(
        lines[1].starts_with(
            "done    sbx exec org-repo -- sh -c <inside_check.sh> sh -t (2 commands): exit status: 0, stdout 26B, stderr 0B "
        ),
        "{lines:?}"
    );
    assert!(ends_with_time(&lines[1], "1244.000ms"), "{lines:?}");
    assert!(
        lines[2].starts_with("inside  0 printenv SSH_AUTH_SOCK: exit status: 1 "),
        "{lines:?}"
    );
    assert!(ends_with_time(&lines[2], "3.102ms"), "{lines:?}");
    assert_eq!(lines[3], "inside  1 ssh-add -L: exit status: 2");
    assert_eq!(lines.len(), 4);
    Ok(())
}

#[test]
fn an_inside_check_that_already_times_itself_is_run_as_given() {
    let clock = clock();
    let commands = vec![vec!["true".to_string()]];
    let spec = inside_check_spec(&commands, true);
    let mut host = SlowHost::new(&clock, Duration::from_millis(1));
    host.inside = Some("0 0 0 0 5000\n".to_string());

    let lines = traced_with(&clock, &host, |traced| {
        let _ = traced.run(&spec);
    });

    assert_eq!(host.seen.borrow()[0].args, spec.args);
    assert_eq!(
        lines[0],
        "run     sbx exec org-repo -- sh -c <inside_check.sh> sh -t (1 command)"
    );
    assert!(
        lines[2].starts_with("inside  0 true: exit status: 0 "),
        "{lines:?}"
    );
}

#[test]
fn an_inside_check_that_did_not_answer_leaves_no_inside_lines() {
    let clock = clock();
    let spec = inside_check_spec(&[vec!["true".to_string()]], false);
    let mut host = SlowHost::new(&clock, Duration::from_millis(1));
    host.inside = Some("not the check's output".to_string());

    let lines = traced_with(&clock, &host, |traced| {
        let _ = traced.run(&spec);
    });

    assert_eq!(lines.len(), 2, "{lines:?}");
}

#[test]
fn a_run_that_returned_no_result_is_traced_with_what_stopped_it() {
    let clock = clock();
    let mut host = SlowHost::new(&clock, Duration::from_millis(10_000));
    host.fails = true;
    let spec = CommandSpec::probe("sbx", &["ls", "--json"]);

    let lines = traced_with(&clock, &host, |traced| {
        let error = traced.run(&spec).err();
        assert!(error.is_some_and(|error| error.contains_id(ErrorId::ExternalCommandTimeout)));
    });

    assert!(
        lines[1].starts_with("failed  sbx ls --json: external-command-timeout "),
        "{lines:?}"
    );
    assert!(ends_with_time(&lines[1], "10000.000ms"), "{lines:?}");
}

#[test]
fn a_terminal_command_is_traced_without_sizes_and_its_output_still_reaches_the_terminal() {
    let clock = clock();
    let command = TerminalCommand::relayed("fake-tool", &["build", "."]);
    let mut output = RecordedOutput::new();
    let lines = traced(&clock, |host| {
        let outcome = host.run_with_terminal(&command, &mut output).ok();
        assert!(outcome.is_some_and(|outcome| outcome.success()));
    });

    assert_eq!(output.text(), "outerr!");
    assert_eq!(lines[0], "run     fake-tool build .");
    assert!(
        lines[1].starts_with("done    fake-tool build .: exit status: 0 "),
        "{lines:?}"
    );
    assert!(ends_with_time(&lines[1], "12.000ms"), "{lines:?}");
}

#[test]
fn a_handed_over_session_follows_the_total_so_far_and_what_runs_during_it_is_not_shown() {
    let clock = clock();
    let every = Duration::from_secs(60);
    let mut host = SlowHost::new(&clock, Duration::from_millis(12));
    // sessionのあいだに保存が2回走る長さ。
    host.session = every * 2 + Duration::from_secs(1);
    let command = TerminalCommand::handed_over("ssh", &["-t", "org-repo"]);
    let save = CommandSpec::capture("git", &["status", "--porcelain"]);
    let mut output = RecordedOutput::new();
    let lines = traced_with(&clock, &host, |traced| {
        let _ = traced.run(&CommandSpec::probe("sbx", &["ls", "--json"]));
        let outcome = traced
            .run_with_terminal_ticking(&command, &mut output, every, &mut || {
                let _ = traced.run(&save);
            })
            .ok();
        assert!(outcome.is_some_and(|outcome| outcome.success()));
    });

    // 保存は走ったが、trace行は渡した端末へ書かれていない。
    assert_eq!(
        host.seen
            .borrow()
            .iter()
            .filter(|spec| spec.program == "git")
            .count(),
        2
    );
    assert!(
        lines[2].starts_with("total   1 external command 12.000ms, "),
        "{lines:?}"
    );
    assert_eq!(lines[3], "run     ssh -t org-repo");
    assert!(
        lines[4].starts_with("session ssh -t org-repo: exit status: 0 "),
        "{lines:?}"
    );
    // sessionは、渡したsshの12msと、その間の保存2回の24msを含む。
    assert!(ends_with_time(&lines[4], "36.000ms"), "{lines:?}");
    assert_eq!(lines.len(), 5, "{lines:?}");
}

#[test]
fn a_handed_over_session_that_returned_no_result_names_what_stopped_it() {
    let clock = clock();
    let mut host = SlowHost::new(&clock, Duration::from_millis(5));
    host.fails = true;
    let command = TerminalCommand::handed_over("ssh", &["-t", "org-repo"]);
    let mut output = RecordedOutput::new();
    let lines = traced_with(&clock, &host, |traced| {
        let error = traced.run_with_terminal(&command, &mut output).err();
        assert!(error.is_some_and(|error| error.contains_id(ErrorId::ExternalCommandTimeout)));
    });

    assert!(
        lines[0].starts_with("total   0 external commands "),
        "{lines:?}"
    );
    assert_eq!(lines[1], "run     ssh -t org-repo");
    assert!(
        lines[2].starts_with("session ssh -t org-repo: external-command-timeout "),
        "{lines:?}"
    );
    assert!(ends_with_time(&lines[2], "5.000ms"), "{lines:?}");
}

#[test]
fn a_handed_over_session_without_ticks_is_a_session_too() {
    let clock = clock();
    let command = TerminalCommand::handed_over("ssh", &["-t", "org-repo"]);
    let mut output = RecordedOutput::new();
    let lines = traced(&clock, |host| {
        let outcome = host.run_with_terminal(&command, &mut output).ok();
        assert!(outcome.is_some_and(|outcome| outcome.success()));
    });

    assert!(
        lines[2].starts_with("session ssh -t org-repo: exit status: 0 "),
        "{lines:?}"
    );
}

#[test]
fn a_relayed_command_run_with_ticks_is_an_external_command() {
    let clock = clock();
    let command = TerminalCommand::relayed("fake-tool", &["build", "."]);
    let mut output = RecordedOutput::new();
    let lines = traced(&clock, |host| {
        let _ = host.run_with_terminal_ticking(
            &command,
            &mut output,
            Duration::from_secs(60),
            &mut || {},
        );
    });

    assert_eq!(lines[0], "run     fake-tool build .");
    assert!(
        lines[1].starts_with("done    fake-tool build .: exit status: 0 "),
        "{lines:?}"
    );
}

#[test]
fn a_streaming_run_counts_the_bytes_it_passed_on() {
    let clock = clock();
    let spec = CommandSpec::capture("git", &["archive", "HEAD"]);
    let mut sink: Vec<u8> = Vec::new();
    let lines = traced(&clock, |host| {
        let outcome = host.run_streaming(&spec, &mut sink, 1024).ok();
        assert!(outcome.is_some_and(|outcome| outcome.success()));
    });

    assert_eq!(sink, b"out");
    assert!(
        lines[1].starts_with("done    git archive HEAD: exit status: 0, stdout 3B, stderr 4B "),
        "{lines:?}"
    );
}

#[test]
fn asking_whether_a_command_exists_is_not_traced() {
    let clock = clock();
    let lines = traced(&clock, |host| {
        assert!(host.command_exists("fake-tool"));
    });

    assert!(lines.is_empty(), "{lines:?}");
}

#[test]
fn arguments_that_a_shell_would_split_are_quoted_and_control_characters_are_shown() {
    let clock = clock();
    let spec = CommandSpec::capture("sh", &["-c", "printf 'a b'", "", "x\ty", "é/ü"]);
    let lines = traced(&clock, |host| {
        let _ = host.run(&spec);
    });

    assert_eq!(
        lines[0],
        concat!("run     sh -c 'printf '\\''a b'\\''' '' 'x\\ty' ", "é/ü")
    );
}

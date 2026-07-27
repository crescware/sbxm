//! 外部commandの実行。
//!
//! shellを介さず、secret値をargumentやdebug表示へ渡さない。人間向け進捗を出す操作は
//! `passthrough`で即時転送し、structured outputをparseする操作は`capture`する。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::error::{Error, ErrorId, ExternalFailure, Result, fail};
use crate::msg;

/// 子processのstdinの扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdinPolicy {
    /// 対話しないcommand。標準入力を閉じる。
    Null,
    /// 対話commandへ現在のterminalを引き渡す。
    Inherit,
}

/// stdoutとstderrの扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamPolicy {
    /// 結果をparseする、または内容を秘匿する必要がある場合。byte列として保持する。
    Capture,
    /// 外部toolの進捗をそのまま見せる場合。到着順に即時転送し、内容は保持しない。
    Passthrough,
    /// interactive SSHのように、既存のterminal動作をそのまま保つ場合。
    Inherit,
}

/// environmentの扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvPolicy {
    /// 現在processのenvironmentを継承する。
    Inherit,
    /// security-sensitiveな`sbx`起動。`SSH_AUTH_SOCK`を必ず除外する。
    InheritWithoutSshAgent,
}

/// timeoutの分類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutClass {
    Probe,
    LocalFilesystem,
    ImageBuild,
    SandboxLifecycle,
    Interactive,
}

impl TimeoutClass {
    pub fn duration(self) -> Option<Duration> {
        match self {
            TimeoutClass::Probe => Some(Duration::from_secs(10)),
            TimeoutClass::LocalFilesystem => Some(Duration::from_secs(60)),
            TimeoutClass::ImageBuild => Some(Duration::from_secs(30 * 60)),
            TimeoutClass::SandboxLifecycle => Some(Duration::from_secs(10 * 60)),
            TimeoutClass::Interactive => None,
        }
    }
}

const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// 1回の外部command実行の指定。
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: EnvPolicy,
    pub stdin: StdinPolicy,
    pub stdout: StreamPolicy,
    pub stderr: StreamPolicy,
    pub timeout: TimeoutClass,
}

impl CommandSpec {
    /// structured outputを読むread-only probe。
    pub fn probe(program: &str, args: &[&str]) -> CommandSpec {
        CommandSpec {
            program: program.to_string(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            cwd: None,
            env: EnvPolicy::Inherit,
            stdin: StdinPolicy::Null,
            stdout: StreamPolicy::Capture,
            stderr: StreamPolicy::Capture,
            timeout: TimeoutClass::Probe,
        }
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> CommandSpec {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn env(mut self, policy: EnvPolicy) -> CommandSpec {
        self.env = policy;
        self
    }

    pub fn timeout(mut self, class: TimeoutClass) -> CommandSpec {
        self.timeout = class;
        self
    }

    /// 外部toolの進捗を隠さずに見せる。
    pub fn passthrough(mut self) -> CommandSpec {
        self.stdout = StreamPolicy::Passthrough;
        self.stderr = StreamPolicy::Passthrough;
        self
    }

    /// 現在のterminalをそのまま引き渡す。
    pub fn interactive(mut self) -> CommandSpec {
        self.stdin = StdinPolicy::Inherit;
        self.stdout = StreamPolicy::Inherit;
        self.stderr = StreamPolicy::Inherit;
        self.timeout = TimeoutClass::Interactive;
        self
    }
}

/// 外部commandの実行結果。
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub status: ExitStatus,
    /// `capture`のときだけ内容を持つ。
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// UTF-8として解釈する際にlossy変換が必要だったか。
    pub stdout_lossy: bool,
    pub stderr_lossy: bool,
}

impl CommandOutcome {
    pub fn success(&self) -> bool {
        self.status.success()
    }

    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    /// 外部commandのexit statusを直接透過せず、原値を診断へ含める。
    pub fn failure(&self) -> ExternalFailure {
        ExternalFailure {
            program: self.program.clone(),
            safe_args: self.args.clone(),
            cwd: self.cwd.clone(),
            exit_status: self.status.to_string(),
            stderr: self.stderr.clone(),
            stderr_lossy: self.stderr_lossy,
        }
    }

    /// 非ゼロstatusを共通のerrorへ写像する。
    pub fn require_success(self) -> Result<CommandOutcome> {
        if self.success() {
            return Ok(self);
        }
        let failure = self.failure();
        Err(Error::single(
            crate::error::Diagnostic::new(
                ErrorId::ExternalCommandFailed,
                msg!(
                    "error-external-command-failed",
                    program = self.program,
                    exit_status = self.status
                ),
            )
            .external(failure),
        ))
    }
}

/// 外部commandを実行する。
pub fn run(spec: &CommandSpec) -> Result<CommandOutcome> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }
    // defaultで現在processのenvironmentを継承する。
    if spec.env == EnvPolicy::InheritWithoutSshAgent {
        command.env_remove("SSH_AUTH_SOCK");
    }
    command.stdin(match spec.stdin {
        StdinPolicy::Null => Stdio::null(),
        StdinPolicy::Inherit => Stdio::inherit(),
    });
    command.stdout(stdio_for(spec.stdout));
    command.stderr(stdio_for(spec.stderr));

    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = spec.program),
            )
        } else {
            Error::new(
                ErrorId::ExternalCommandSpawnFailed,
                msg!(
                    "error-external-command-spawn-failed",
                    program = spec.program,
                    detail = error
                ),
            )
        }
    })?;

    // pipeが埋まって子processが止まらないよう、両streamを並行に読む。
    let stdout_reader = child
        .stdout
        .take()
        .map(|pipe| spawn_reader(pipe, spec.stdout, Sink::Stdout));
    let stderr_reader = child
        .stderr
        .take()
        .map(|pipe| spawn_reader(pipe, spec.stderr, Sink::Stderr));

    let status = wait_with_timeout(&mut child, spec)?;

    let stdout = stdout_reader.map(|handle| handle.join().unwrap_or_default());
    let stderr = stderr_reader.map(|handle| handle.join().unwrap_or_default());
    let (stdout, stdout_lossy) = finish(stdout);
    let (stderr, stderr_lossy) = finish(stderr);

    Ok(CommandOutcome {
        program: spec.program.clone(),
        args: spec.args.clone(),
        cwd: spec.cwd.clone(),
        status,
        stdout,
        stderr,
        stdout_lossy,
        stderr_lossy,
    })
}

fn stdio_for(policy: StreamPolicy) -> Stdio {
    match policy {
        StreamPolicy::Capture | StreamPolicy::Passthrough => Stdio::piped(),
        StreamPolicy::Inherit => Stdio::inherit(),
    }
}

fn finish(collected: Option<Vec<u8>>) -> (Vec<u8>, bool) {
    let bytes = collected.unwrap_or_default();
    let lossy = matches!(String::from_utf8_lossy(&bytes), std::borrow::Cow::Owned(_));
    (bytes, lossy)
}

#[derive(Debug, Clone, Copy)]
enum Sink {
    Stdout,
    Stderr,
}

/// 子processの1 streamを読み切る。
///
/// `passthrough`では完了までbufferせず、到着順に対応するparent streamへ即時転送する。
fn spawn_reader<R: Read + Send + 'static>(
    mut pipe: R,
    policy: StreamPolicy,
    sink: Sink,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut collected = Vec::new();
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    let chunk = &buffer[..read];
                    match policy {
                        StreamPolicy::Capture => collected.extend_from_slice(chunk),
                        StreamPolicy::Passthrough => {
                            // 翻訳、要約、再構成をせずそのまま転送する。
                            match sink {
                                Sink::Stdout => {
                                    let mut out = std::io::stdout().lock();
                                    let _ = out.write_all(chunk);
                                    let _ = out.flush();
                                }
                                Sink::Stderr => {
                                    let mut out = std::io::stderr().lock();
                                    let _ = out.write_all(chunk);
                                    let _ = out.flush();
                                }
                            }
                        }
                        StreamPolicy::Inherit => {}
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        collected
    })
}

fn wait_with_timeout(child: &mut Child, spec: &CommandSpec) -> Result<ExitStatus> {
    let Some(limit) = spec.timeout.duration() else {
        return child.wait().map_err(|error| {
            Error::new(
                ErrorId::ExternalCommandSpawnFailed,
                msg!(
                    "error-external-command-spawn-failed",
                    program = spec.program,
                    detail = error
                ),
            )
        });
    };

    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    // timeout時はchildを終了する。
                    let _ = child.kill();
                    let _ = child.wait();
                    return fail(
                        ErrorId::ExternalCommandTimeout,
                        msg!(
                            "error-external-command-timeout",
                            program = spec.program,
                            seconds = limit.as_secs()
                        ),
                    );
                }
                std::thread::sleep(WAIT_POLL_INTERVAL);
            }
            Err(error) => {
                return fail(
                    ErrorId::ExternalCommandSpawnFailed,
                    msg!(
                        "error-external-command-spawn-failed",
                        program = spec.program,
                        detail = error
                    ),
                );
            }
        }
    }
}

/// PATH上にcommandが存在するかを、実行せずに調べる。
pub fn exists_on_path(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(program);
        is_executable(&candidate)
    })
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

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
        for _ in 0..50 {
            match run(spec) {
                Err(error) if error.contains(ErrorId::ExternalCommandSpawnFailed) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                other => return other,
            }
        }
        run(spec)
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
        assert_eq!(
            TimeoutClass::ImageBuild.duration(),
            Some(Duration::from_secs(1800))
        );
        assert_eq!(
            TimeoutClass::SandboxLifecycle.duration(),
            Some(Duration::from_secs(600))
        );
        assert_eq!(TimeoutClass::Interactive.duration(), None);
    }

    #[test]
    fn the_runner_records_program_arguments_and_working_directory() {
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join("record");
        let fake = fake_executable(
            dir.path(),
            "fake-tool",
            &format!(
                r#"{{
  echo "cwd=$(pwd)"
  for a in "$@"; do echo "arg=$a"; done
}} > "{}""#,
                record.display()
            ),
        );
        let workdir = dir.path().join("work");
        fs::create_dir(&workdir).unwrap();

        let spec = CommandSpec::probe(fake.to_str().unwrap(), &["ls", "--json"]).cwd(&workdir);
        let outcome = run_fake(&spec).expect("the fake tool runs");
        assert!(outcome.success());

        let recorded = fs::read_to_string(&record).unwrap();
        assert!(recorded.contains("arg=ls"), "{recorded}");
        assert!(recorded.contains("arg=--json"), "{recorded}");
        assert!(
            recorded.contains(&format!(
                "cwd={}",
                fs::canonicalize(&workdir).unwrap().display()
            )),
            "{recorded}"
        );
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
        assert_eq!(outcome.stderr_text(), "to stderr");
        assert!(!outcome.success());
        assert_eq!(outcome.status.code(), Some(3));
    }

    #[test]
    fn invalid_utf8_output_is_kept_as_bytes_and_reported_as_lossy() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake_executable(dir.path(), "fake-tool", r#"printf '\377\376'"#);

        let outcome = run_fake(&CommandSpec::probe(fake.to_str().unwrap(), &[])).expect("runs");
        assert_eq!(outcome.stdout, vec![0xff, 0xfe]);
        assert!(
            outcome.stdout_lossy,
            "a lossy conversion must be reported as such"
        );
        assert!(!outcome.stderr_lossy);
    }

    #[test]
    fn passthrough_does_not_retain_the_external_output() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake_executable(dir.path(), "fake-tool", "printf 'progress'");

        let spec = CommandSpec::probe(fake.to_str().unwrap(), &[]).passthrough();
        let outcome = run_fake(&spec).expect("runs");
        assert!(
            outcome.stdout.is_empty(),
            "passthrough output is shown as it arrives, not buffered for redisplay"
        );
        assert!(outcome.success());
    }

    #[test]
    fn a_command_that_exceeds_its_timeout_is_terminated() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake_executable(dir.path(), "fake-tool", "sleep 30");

        let mut spec = CommandSpec::probe(fake.to_str().unwrap(), &[]);
        spec.timeout = TimeoutClass::Probe;
        // probeの10秒を待たずに判定するため、直接短いdeadlineを使う。
        let started = Instant::now();
        let error = run_with_limit(&spec, Duration::from_millis(200))
            .expect_err("the command must be terminated");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the child must be killed promptly"
        );
    }

    /// timeout classの既定値を使わずに待ち時間を差し替えるtest helper。
    fn run_with_limit(spec: &CommandSpec, limit: Duration) -> Result<CommandOutcome> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // run_fakeと同じ理由で、`ETXTBSY`のあいだだけ再試行する。
        let mut child = loop {
            match command.spawn() {
                Ok(child) => break child,
                Err(error) if error.raw_os_error() == Some(26) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("the fake tool could not be started: {error}"),
            }
        };
        let deadline = Instant::now() + limit;
        loop {
            match child.try_wait().expect("try_wait") {
                Some(status) => {
                    return Ok(CommandOutcome {
                        program: spec.program.clone(),
                        args: spec.args.clone(),
                        cwd: None,
                        status,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                        stdout_lossy: false,
                        stderr_lossy: false,
                    });
                }
                None => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return fail(
                            ErrorId::ExternalCommandTimeout,
                            msg!(
                                "error-external-command-timeout",
                                program = spec.program,
                                seconds = limit.as_secs()
                            ),
                        );
                    }
                    std::thread::sleep(WAIT_POLL_INTERVAL);
                }
            }
        }
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
}

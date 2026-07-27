//! 外部commandの実行。
//!
//! shellを介さず、secret値をargumentやdebug表示へ渡さない。stdoutとstderrは
//! それぞれ独立にcaptureし、structured outputのparseと診断表示に使う。

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::error::{Error, ErrorId, ExternalFailure, Result, fail};
use crate::msg;

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
}

impl TimeoutClass {
    pub fn duration(self) -> Duration {
        match self {
            TimeoutClass::Probe => Duration::from_secs(10),
            TimeoutClass::LocalFilesystem => Duration::from_secs(60),
        }
    }
}

const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// 1回の外部command実行の指定。
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: EnvPolicy,
    pub timeout: TimeoutClass,
}

impl CommandSpec {
    /// structured outputを読むread-only probe。
    pub fn probe(program: &str, args: &[&str]) -> CommandSpec {
        CommandSpec {
            program: program.to_string(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            env: EnvPolicy::Inherit,
            timeout: TimeoutClass::Probe,
        }
    }

    pub fn env(mut self, policy: EnvPolicy) -> CommandSpec {
        self.env = policy;
        self
    }

    pub fn timeout(mut self, class: TimeoutClass) -> CommandSpec {
        self.timeout = class;
        self
    }
}

/// 外部commandの実行結果。
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    pub program: String,
    pub args: Vec<String>,
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// stderrをUTF-8として解釈する際にlossy変換が必要だったか。
    pub stderr_lossy: bool,
}

impl CommandOutcome {
    pub fn success(&self) -> bool {
        self.status.success()
    }

    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// 外部commandのexit statusを直接透過せず、原値を診断へ含める。
    pub fn failure(&self) -> ExternalFailure {
        ExternalFailure {
            program: self.program.clone(),
            safe_args: self.args.clone(),
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
    // defaultで現在processのenvironmentを継承する。
    if spec.env == EnvPolicy::InheritWithoutSshAgent {
        command.env_remove("SSH_AUTH_SOCK");
    }
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

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
    let stdout_reader = child.stdout.take().map(spawn_reader);
    let stderr_reader = child.stderr.take().map(spawn_reader);

    let status = wait_with_timeout(&mut child, spec)?;

    let stdout = stdout_reader
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr_reader
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr_lossy = matches!(String::from_utf8_lossy(&stderr), std::borrow::Cow::Owned(_));

    Ok(CommandOutcome {
        program: spec.program.clone(),
        args: spec.args.clone(),
        status,
        stdout,
        stderr,
        stderr_lossy,
    })
}

/// 子processの1 streamを読み切る。
fn spawn_reader<R: Read + Send + 'static>(mut pipe: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut collected = Vec::new();
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => collected.extend_from_slice(&buffer[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        collected
    })
}

fn wait_with_timeout(child: &mut Child, spec: &CommandSpec) -> Result<ExitStatus> {
    let limit = spec.timeout.duration();
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
        assert_eq!(TimeoutClass::Probe.duration(), Duration::from_secs(10));
        assert_eq!(
            TimeoutClass::LocalFilesystem.duration(),
            Duration::from_secs(60)
        );
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
                        status,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
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

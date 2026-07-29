//! 外部commandの実行。
//!
//! shellを介さず、secret値をargumentやdebug表示へ渡さない。stdoutとstderrは
//! それぞれ独立にcaptureし、structured outputのparseと診断表示に使う。

use std::io::Read;
use std::path::{Path, PathBuf};
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
    /// `docker build`と`docker image save`。base imageのpullとpackage導入を含む。
    ImageBuild,
    /// Template load、Sandboxの作成・起動・停止、Sandbox内commandの実行。
    SandboxLifecycle,
    /// Git cloneとfetch。転送量がrepositoryの大きさで決まる。
    RepositoryTransfer,
    /// 利用者が操作している対話接続。終わる時期を決めるのは利用者である。
    Interactive,
}

impl TimeoutClass {
    /// 待機の上限。対話中の接続にはtimeoutを課さない。
    pub fn duration(self) -> Option<Duration> {
        match self {
            TimeoutClass::Probe => Some(Duration::from_secs(10)),
            TimeoutClass::LocalFilesystem => Some(Duration::from_secs(60)),
            TimeoutClass::ImageBuild => Some(Duration::from_secs(1800)),
            TimeoutClass::SandboxLifecycle => Some(Duration::from_secs(600)),
            TimeoutClass::RepositoryTransfer => Some(Duration::from_secs(1800)),
            TimeoutClass::Interactive => None,
        }
    }
}

/// 子processの出力の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputPolicy {
    /// stdoutとstderrを別々にbyte列としてcaptureする。
    ///
    /// 結果をparseする、または内容を秘匿するcommandへ使う。
    Capture,
    /// 人間向けの進捗を、外部toolが出したまま端末へ転送する。
    ///
    /// 長時間かかる工程の進捗を実行中に見せるために使う。sbxmは進捗の実況を重ねない。
    /// 何も出さない外部commandについては、工程の開始を`progress`が1行で予告する。
    /// captureしないため、失敗の診断にstderrの原文は含まれない。
    Passthrough,
    /// terminalそのものを引き渡す。
    ///
    /// SSH接続のように、利用者が入力する対話processへ使う。stdinも継承するため、
    /// 既存のterminal動作がそのまま保たれる。
    Inherit,
}

const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// 1回の外部command実行の指定。
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: EnvPolicy,
    pub timeout: TimeoutClass,
    pub output: OutputPolicy,
    /// 作業directory。指定しない場合は現在processのcurrent directoryを継承する。
    pub working_dir: Option<PathBuf>,
}

impl CommandSpec {
    /// structured outputを読むread-only probe。
    pub fn probe(program: &str, args: &[&str]) -> CommandSpec {
        CommandSpec::capture(program, args).timeout(TimeoutClass::Probe)
    }

    /// 出力をparseする、または秘匿するcommand。
    pub fn capture(program: &str, args: &[&str]) -> CommandSpec {
        CommandSpec {
            program: program.to_string(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            env: EnvPolicy::Inherit,
            timeout: TimeoutClass::Probe,
            output: OutputPolicy::Capture,
            working_dir: None,
        }
    }

    /// 進捗をそのまま見せるcommand。
    pub fn passthrough(program: &str, args: &[&str]) -> CommandSpec {
        CommandSpec {
            output: OutputPolicy::Passthrough,
            ..CommandSpec::capture(program, args)
        }
    }

    /// terminalを引き渡す対話command。
    pub fn inherit(program: &str, args: &[&str]) -> CommandSpec {
        CommandSpec {
            output: OutputPolicy::Inherit,
            timeout: TimeoutClass::Interactive,
            ..CommandSpec::capture(program, args)
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

    pub fn working_dir(mut self, directory: &Path) -> CommandSpec {
        self.working_dir = Some(directory.to_path_buf());
        self
    }
}

/// 外部commandの実行結果。
///
/// `passthrough`で実行した場合、出力は既に端末へ渡っているため両streamは空になる。
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
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
            working_dir: self.working_dir.clone(),
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
    run_inner(spec, spec.timeout.duration())
}

/// timeout classの既定値ではない待ち時間で実行する。
///
/// 最短のclassでも10秒あるため、deadlineに達する側の分岐はtestからしか踏めない。
#[cfg(test)]
pub fn run_with_limit(spec: &CommandSpec, limit: Duration) -> Result<CommandOutcome> {
    run_inner(spec, Some(limit))
}

fn run_inner(spec: &CommandSpec, limit: Option<Duration>) -> Result<CommandOutcome> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    // defaultで現在processのenvironmentを継承する。
    if spec.env == EnvPolicy::InheritWithoutSshAgent {
        command.env_remove("SSH_AUTH_SOCK");
    }
    if let Some(directory) = &spec.working_dir {
        command.current_dir(directory);
    }
    match spec.output {
        OutputPolicy::Capture => {
            command.stdin(Stdio::null());
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
        }
        OutputPolicy::Passthrough => {
            command.stdin(Stdio::null());
            command.stdout(Stdio::inherit());
            command.stderr(Stdio::inherit());
        }
        OutputPolicy::Inherit => {
            // 利用者の入力をそのまま渡す。
            command.stdin(Stdio::inherit());
            command.stdout(Stdio::inherit());
            command.stderr(Stdio::inherit());
        }
    }

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

    let status = wait_with_limit(&mut child, spec, limit)?;

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
        working_dir: spec.working_dir.clone(),
        status,
        stdout,
        stderr,
        stderr_lossy,
    })
}

/// hostに対する外部commandの実行。testでは差し替える。
pub trait HostEnvironment {
    fn command_exists(&self, program: &str) -> bool;
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome>;
}

/// 実際のhost。
pub struct RealHost;

impl HostEnvironment for RealHost {
    fn command_exists(&self, program: &str) -> bool {
        exists_on_path(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        run(spec)
    }
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

fn wait_with_limit(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
) -> Result<ExitStatus> {
    let Some(limit) = limit else {
        // 対話processは、利用者が終えるまで待つ。
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
#[path = "command_test.rs"]
mod command_test;

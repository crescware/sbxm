use std::path::PathBuf;
use std::process::ExitStatus;

use crate::diagnostics::{Error, ErrorId, ExternalFailure, Result};
use crate::msg;

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
            crate::diagnostics::Diagnostic::new(
                ErrorId::ExternalCommandFailed,
                // programは事実の行が示すため、説明文へは繰り返さない。
                msg!("error-external-command-failed", exit_status = self.status),
            )
            .external(failure),
        ))
    }
}

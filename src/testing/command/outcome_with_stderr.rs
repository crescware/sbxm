use crate::boundary::host::{CommandOutcome, CommandSpec};
use std::os::unix::process::ExitStatusExt;

/// 標準error出力も持つ実行結果。
pub fn outcome_with_stderr(
    spec: &CommandSpec,
    code: i32,
    stdout: &str,
    stderr: &str,
) -> CommandOutcome {
    CommandOutcome {
        program: spec.program.clone(),
        args: spec.args.clone(),
        working_dir: spec.working_dir.clone(),
        // waitpid由来の値と同じく、終了codeは上位byteに置く。
        status: std::process::ExitStatus::from_raw(code << 8),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
        stderr_lossy: false,
    }
}

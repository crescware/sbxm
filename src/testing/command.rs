//! 外部commandの偽の応答。

use crate::command::{CommandOutcome, CommandSpec};
use std::os::unix::process::ExitStatusExt;

/// 指定と終了codeと標準出力から、実行結果を組み立てる。
pub fn outcome(spec: &CommandSpec, code: i32, stdout: &str) -> CommandOutcome {
    outcome_with_stderr(spec, code, stdout, "")
}

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

/// `sbx exec <name> -- ...`の`--`より後ろ。`--`がなければ空。
pub fn inner_args(spec: &CommandSpec) -> Vec<&str> {
    spec.args
        .iter()
        .skip_while(|arg| *arg != "--")
        .skip(1)
        .map(String::as_str)
        .collect()
}

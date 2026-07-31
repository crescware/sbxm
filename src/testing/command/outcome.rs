use crate::command::{CommandOutcome, CommandSpec};

use super::outcome_with_stderr;

/// 指定と終了codeと標準出力から、実行結果を組み立てる。
pub fn outcome(spec: &CommandSpec, code: i32, stdout: &str) -> CommandOutcome {
    outcome_with_stderr(spec, code, stdout, "")
}

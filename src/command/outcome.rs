use super::{CommandOutcome, CommandSpec};

/// 実行の結果を、何を起動したかと一緒に組み立てる。
pub(super) fn outcome(
    spec: &CommandSpec,
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
) -> CommandOutcome {
    let stderr_lossy = matches!(String::from_utf8_lossy(&stderr), std::borrow::Cow::Owned(_));
    CommandOutcome {
        program: spec.program.clone(),
        args: spec.args.clone(),
        working_dir: spec.working_dir.clone(),
        status,
        stdout,
        stderr,
        stderr_lossy,
    }
}

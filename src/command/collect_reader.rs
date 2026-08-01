use std::thread::JoinHandle;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::CommandSpec;

/// reader threadを1つ回収する。
///
/// 子processがどう終わったかに関わらず必ず呼ぶ。回収しないまま返ると、pipeを読んでいる
/// threadが実行の外側へ残り、いつ終わるかを誰も知らない状態になる。
pub(super) fn collect_reader(
    reader: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
    spec: &CommandSpec,
) -> Result<Vec<u8>> {
    // 端末へ出すpolicyはpipeを開かないため、回収するreaderも無い。
    let Some(handle) = reader else {
        return Ok(Vec::new());
    };
    match handle.join() {
        Ok(Ok(collected)) => Ok(collected),
        Ok(Err(error)) => Err(unreadable(spec, Fact::cause(&error.to_string()))),
        // threadが落ちた場合、読めていた分も一緒に失われている。
        Err(_) => Err(unreadable(
            spec,
            Fact::reason(msg!("cause-output-reader-ended")),
        )),
    }
}

/// 出力を最後まで読めなかったことを報告する。
fn unreadable(spec: &CommandSpec, cause: Fact) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandOutputUnreadable,
            msg!("error-external-command-output-unreadable"),
        )
        .fact(Fact::command(&spec.program))
        .fact(cause),
    )
}

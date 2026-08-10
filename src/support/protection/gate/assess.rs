use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::super::{Assessment, Request, inspect};

/// worktree、Git操作、origin回収可能性を固定順序で評価する。
///
/// 呼び出し側は個々のcollectorを選べない。観測そのものに失敗した場合も観測不能の
/// blockerとして`Assessment`へ収め、`gate::authorize`で他の拒否理由と一緒に表示する。
// 既存の呼び出し側が共通のdiagnostic flowを使えるよう、Resultの境界は残す。観測失敗
// 自体はAssessment内で表現する。
#[allow(clippy::unnecessary_wraps)]
pub fn assess(host: &dyn HostEnvironment, request: &Request<'_>) -> Result<Assessment> {
    Ok(inspect::inspect(host, request))
}

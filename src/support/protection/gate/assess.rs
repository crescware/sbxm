use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::super::{ProtectionAssessment, ProtectionRequest, inspect};

/// worktree、Git操作、origin回収可能性を固定順序で評価する。
///
/// 呼び出し側は個々のcollectorを選べない。観測そのものに失敗した場合は`Err`とし、
/// この場合`gate::authorize`へは進めない。
///
/// `request`は呼び出し側が一度だけ組み立てて渡すopaqueな入力であり、値渡しで
/// 受け取ることを公開契約として固定する。
#[allow(clippy::needless_pass_by_value)]
pub fn assess(
    host: &dyn HostEnvironment,
    request: ProtectionRequest<'_>,
) -> Result<ProtectionAssessment> {
    inspect::inspect(host, &request)
}

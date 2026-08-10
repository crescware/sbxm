use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::super::{Assessment, Request, inspect};

/// worktree、Git操作、origin回収可能性を固定順序で評価する。
///
/// 呼び出し側は個々のcollectorを選べない。観測そのものに失敗した場合は`Err`とし、
/// この場合`gate::authorize`へは進めない。
pub fn assess(host: &dyn HostEnvironment, request: &Request<'_>) -> Result<Assessment> {
    inspect::inspect(host, request)
}

use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::super::{ProtectionSnapshot, Request, inspect};

/// worktree、Git操作、origin回収可能性を固定順序で評価し、その瞬間の観測結果を返す。
///
/// 呼び出し側は個々のcollectorを選べない。観測そのものに失敗した場合も観測不能の
/// blockerとして`Assessment`へ収め、`gate::require_no_blockers`で他の拒否理由と一緒に
/// 表示する。
///
/// `ProtectionSnapshot`を作れるのはこの関数と[`super::assess_absent`]だけである。
/// 呼び出し側が任意の`Assessment`からsnapshotを組み立てられると、観測を1回も行わずに
/// 一致するconfirmationとpermitを作れてしまう。
// 既存の呼び出し側が共通のdiagnostic flowを使えるよう、Resultの境界は残す。観測失敗
// 自体はAssessment内で表現する。
#[allow(clippy::unnecessary_wraps)]
pub fn assess(host: &dyn HostEnvironment, request: &Request<'_>) -> Result<ProtectionSnapshot> {
    Ok(ProtectionSnapshot::new(inspect::inspect(host, request)))
}

use crate::diagnostics::{Error, Result};

use super::super::Assessment;

/// blockerが1件もない場合だけ通常経路を先へ進める。
///
/// blockerが1件でもあれば、既知の全件を安定順序で1つの`Error::Diagnostics`へ変換して
/// 拒否する。1件目で打ち切って、別のblockerを再実行のたびに小出しにはしない。
///
/// `assessment`をconsumeし、呼び出し側が同じbindingを保持したまま複数回authorizeへ
/// 渡すことを防ぐ。判定を再び必要とする場面では、呼び出し側が新しく`gate::assess`を
/// 呼ぶ。
#[allow(clippy::needless_pass_by_value)]
pub fn authorize(assessment: Assessment) -> Result<()> {
    if assessment.blockers().is_empty() {
        return Ok(());
    }
    Err(Error::Diagnostics(
        assessment
            .blockers()
            .iter()
            .map(|blocker| blocker.diagnostic(assessment.project()))
            .collect(),
    ))
}

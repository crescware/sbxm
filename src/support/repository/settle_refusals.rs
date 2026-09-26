use crate::design::{Fact, ProgressSink, Warning};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::PushRefusal;

/// Sandboxのoriginへ書き込んだとき、gitが断ったrefの扱いを決める。
///
/// tagは上書きしないため、同じ名前で別の先を指すtagは断られる。Sandboxで作ったtagを
/// 消さないよう、Sandboxの側を残してwarningで示す。branchは強制して書き込むため、
/// 断ったのはSandboxの中の何か（repositoryのhookなど）である。古い`origin/*`のまま
/// worktreeを作らないよう、失敗にする。
pub(super) fn settle_refusals(
    sandbox: &str,
    refused: &[PushRefusal],
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    let (tags, branches): (Vec<&PushRefusal>, Vec<&PushRefusal>) = refused
        .iter()
        .partition(|refusal| refusal.reference.starts_with("refs/tags/"));
    if !branches.is_empty() {
        let mut diagnostic = Diagnostic::new(
            ErrorId::SandboxRepositoryUnwritable,
            msg!("error-sandbox-repository-unwritable", sandbox = sandbox),
        );
        for refusal in branches {
            diagnostic = diagnostic
                .fact(Fact::reference(&refusal.reference))
                .fact(Fact::cause(&refusal.reason));
        }
        return Err(Error::single(
            diagnostic.remediation(msg!("remediation-sandbox-origin-refused")),
        ));
    }
    if !tags.is_empty() {
        let mut warning = Warning::text(msg!("warning-sandbox-tags-kept", sandbox = sandbox));
        for refusal in tags {
            warning = warning
                .fact(Fact::reference(&refusal.reference))
                .fact(Fact::cause(&refusal.reason));
        }
        progress.warn(warning);
    }
    Ok(())
}

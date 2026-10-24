use crate::boundary::host::protocol::SandboxEntry;
use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::design::{Fact, ProgressSink};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::support::daemon;

/// 認証を必要とするread-only commandで、案件選択前にloginを確認する。
///
/// `sbx login`にstatus subcommandはないため、`ls --json`を短いprobeとして使う。
/// probeで読んだ一覧を返す。案件のlockを取るworkflowは、これを使わずにlock取得後に
/// 現在の状態を取り直す。lockもpromptも挟まずに一覧を読むworkflow（`ls`）は、同じ
/// 一覧を読み直さずにこれを使う。
pub fn require_signed_in(
    host: &dyn HostEnvironment,
    progress: &mut dyn ProgressSink,
) -> Result<Vec<SandboxEntry>> {
    progress.step(msg!("progress-inspect-login"));
    daemon::list_with_timeout(host, TimeoutClass::Probe).map_err(unobservable)
}

fn unobservable(error: Error) -> Error {
    if error == Error::Canceled
        || error.contains_id(ErrorId::SbxLoginMissing)
        || error.contains_id(ErrorId::ExternalOutputUnparseable)
    {
        return error;
    }
    let mut diagnostic = Diagnostic::new(
        ErrorId::SbxLoginUnobservable,
        msg!("error-sbx-login-unobservable"),
    );
    if let Some(source) = error.diagnostics().first() {
        diagnostic = diagnostic.fact(Fact::cause(source.id.as_str()));
        if let Some(external) = &source.external {
            diagnostic = diagnostic.external(external.clone());
        }
    }
    Error::single(diagnostic)
}

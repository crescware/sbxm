use crate::boundary::host::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::Error;
use crate::msg;

use super::version_probe;

/// docker関連の失敗に、daemonへの疎通を再確認した結果を付け加える。
///
/// 事前の疎通確認では、commandの実行中にdaemonが不安定になる事故を検知できない。
/// 曖昧さは危険側に倒す: 再probeが明確に成功したという積極的な証拠がない限り
/// (timeoutや別の失敗も含めて)、daemonが無関係だったとは推測しない。
pub fn diagnose_failure(host: &dyn HostEnvironment, error: Error) -> Error {
    let Error::Diagnostics(diagnostics) = &error else {
        // Canceledは利用者の中断であり、付け加える対象がない。
        return error;
    };
    if probe_says_reachable(host) {
        return error;
    }
    Error::Diagnostics(
        diagnostics
            .iter()
            .cloned()
            .map(|diagnostic| {
                diagnostic
                    .fact(Fact::reason(msg!("cause-docker-daemon-also-unreachable")))
                    .remediation(msg!("remediation-start-docker"))
            })
            .collect(),
    )
}

/// 再probeが明確に成功したか。timeoutや別の失敗は「不明」であり、成功とは扱わない。
fn probe_says_reachable(host: &dyn HostEnvironment) -> bool {
    matches!(
        version_probe(host),
        Ok(outcome) if outcome.success() && !outcome.stdout_text().trim().is_empty()
    )
}

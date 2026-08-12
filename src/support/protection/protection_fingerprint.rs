use serde::Serialize;

use crate::hash::sha256_hex;

use super::Assessment;

/// fingerprint入力形の版識別子。入力に含める項目を変えたら値を上げる。
const FINGERPRINT_VERSION: &str = "sbxm-protection-v1";

/// 確認対象の状態を表すSHA-256値。
///
/// 内部表現とconstructorは非公開であり、[`super::ProtectionSnapshot::new`]だけが
/// 生成できる。表示形式は小文字16進64文字だが、利用者に入力させる値には使わない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionFingerprint {
    hex: String,
}

impl ProtectionFingerprint {
    /// `assessment`から正規化した入力を作り、そのSHA-256を求める。
    ///
    /// 収集順に依存しないよう、worktree・拒否理由（`Blocker`）・確認対象
    /// （`ConfirmableLoss`）をそれぞれ安定した文字列表現へ写してから昇順に並べ替える。
    /// 表示文、翻訳済み文字列、remote URL、credential、file内容は入力に含めない
    /// （そもそも`Blocker`/`ConfirmableLoss`はこれらを保持しない）。
    pub(super) fn of(assessment: &Assessment) -> ProtectionFingerprint {
        let mut worktrees: Vec<WorktreeInput> = assessment
            .worktrees()
            .iter()
            .map(|report| WorktreeInput {
                relative: report.relative.clone(),
                kind: report.kind.as_str(),
                mode: report.mode.as_str(),
                head: report.head.clone(),
                branch: report.branch.clone(),
                remote: report.remote.as_str(),
            })
            .collect();
        worktrees.sort_by(|a, b| a.relative.cmp(&b.relative));

        let mut blockers: Vec<String> = assessment
            .blockers()
            .iter()
            .map(|blocker| format!("{blocker:?}"))
            .collect();
        blockers.sort();

        let mut confirmable_losses: Vec<String> = assessment
            .confirmable_losses()
            .iter()
            .map(|loss| format!("{loss:?}"))
            .collect();
        confirmable_losses.sort();

        let input = FingerprintInput {
            version: FINGERPRINT_VERSION,
            operation: assessment.operation().as_str(),
            sandbox: assessment.sandbox().as_str().to_string(),
            worktrees,
            blockers,
            confirmable_losses,
        };
        let bytes = serde_json::to_vec(&input).unwrap_or_default();
        ProtectionFingerprint {
            hex: sha256_hex(&bytes),
        }
    }
}

/// 正規化した、fingerprint計算専用のDTO。
#[derive(Serialize)]
struct FingerprintInput {
    version: &'static str,
    operation: &'static str,
    sandbox: String,
    worktrees: Vec<WorktreeInput>,
    blockers: Vec<String>,
    confirmable_losses: Vec<String>,
}

#[derive(Serialize)]
struct WorktreeInput {
    relative: String,
    kind: &'static str,
    mode: &'static str,
    head: String,
    branch: Option<String>,
    remote: &'static str,
}

#[cfg(test)]
#[path = "protection_fingerprint_test.rs"]
mod protection_fingerprint_test;

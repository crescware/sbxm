use serde::Serialize;

use crate::hash::sha256_hex;

use super::{Assessment, Blocker, ConfirmableLoss, OriginObservation};

/// fingerprint入力形の版識別子。入力に含める項目を変えたら値を上げる。
const FINGERPRINT_VERSION: &str = "sbxm-protection-v2";

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
    /// 写し先は`Debug`表現ではなく、variantの識別子と識別に要るfieldだけを並べた
    /// `fingerprint_key`である。`Blocker::Unobservable`が保持する`Diagnostic`は外部
    /// commandのstderrを含むため、`Debug`をそのまま入力にすると表示文や外部outputが
    /// 混じる。`fingerprint_key`は`ErrorId`だけを写し、表示文、翻訳済み文字列、
    /// remote URL、credential、file内容を入力から外す。
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
                reachability: report.reachability.fingerprint_key(),
            })
            .collect();
        worktrees.sort_by(|a, b| a.relative.cmp(&b.relative));

        let mut blockers: Vec<String> = assessment
            .blockers()
            .iter()
            .map(Blocker::fingerprint_key)
            .collect();
        blockers.sort();

        let mut confirmable_losses: Vec<String> = assessment
            .confirmable_losses()
            .iter()
            .map(ConfirmableLoss::fingerprint_key)
            .collect();
        confirmable_losses.sort();

        let origin_observation = assessment
            .origin_observation()
            .map(OriginObservation::fingerprint_key);

        let input = FingerprintInput {
            version: FINGERPRINT_VERSION,
            operation: assessment.operation().as_str(),
            sandbox: assessment.sandbox().as_str().to_string(),
            worktrees,
            blockers,
            confirmable_losses,
            origin_observation,
        };
        // `FingerprintInput`は文字列とその列だけで構成しており、直列化は失敗しない。
        // それでも空byte列へ丸めると、あらゆる状態が同じhashへ潰れて状態変化の検出が
        // 成り立たなくなる。fallbackも状態ごとに異なる表現にし、同一視だけは起こさない。
        let bytes =
            serde_json::to_vec(&input).unwrap_or_else(|_| format!("{input:?}").into_bytes());
        ProtectionFingerprint {
            hex: sha256_hex(&bytes),
        }
    }
}

/// 正規化した、fingerprint計算専用のDTO。
#[derive(Debug, Serialize)]
struct FingerprintInput {
    version: &'static str,
    operation: &'static str,
    sandbox: String,
    worktrees: Vec<WorktreeInput>,
    blockers: Vec<String>,
    confirmable_losses: Vec<String>,
    /// origin観測結果。確認後のorigin側の変化を古いpermitで越えられないようにする。
    origin_observation: Option<String>,
}

#[derive(Debug, Serialize)]
struct WorktreeInput {
    relative: String,
    kind: &'static str,
    mode: &'static str,
    head: String,
    branch: Option<String>,
    reachability: String,
}

#[cfg(test)]
#[path = "protection_fingerprint_test.rs"]
mod protection_fingerprint_test;

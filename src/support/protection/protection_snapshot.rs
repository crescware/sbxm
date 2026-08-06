use super::{ProtectionAssessment, ProtectionFingerprint};

/// `ProtectionAssessment`と、その内容から作った`ProtectionFingerprint`を持つ、
/// ある瞬間の観測結果。
///
/// `new`だけがfingerprintを生成する。field は非公開とし、表示用のread-only
/// accessorだけを公開する。
#[derive(Debug)]
pub struct ProtectionSnapshot {
    pub(super) assessment: ProtectionAssessment,
    pub(super) fingerprint: ProtectionFingerprint,
}

impl ProtectionSnapshot {
    pub fn new(assessment: ProtectionAssessment) -> ProtectionSnapshot {
        let fingerprint = ProtectionFingerprint::of(&assessment);
        ProtectionSnapshot {
            assessment,
            fingerprint,
        }
    }

    pub fn assessment(&self) -> &ProtectionAssessment {
        &self.assessment
    }

    pub fn fingerprint(&self) -> &ProtectionFingerprint {
        &self.fingerprint
    }
}

use super::{Assessment, ProtectionFingerprint};

/// `Assessment`と、その内容から作った`ProtectionFingerprint`を持つ、
/// ある瞬間の観測結果。
///
/// `new`だけがfingerprintを生成する。field は非公開とし、表示用のread-only
/// accessorだけを公開する。
#[derive(Debug)]
pub struct ProtectionSnapshot {
    pub(super) assessment: Assessment,
    pub(super) fingerprint: ProtectionFingerprint,
}

impl ProtectionSnapshot {
    pub fn new(assessment: Assessment) -> ProtectionSnapshot {
        let fingerprint = ProtectionFingerprint::of(&assessment);
        ProtectionSnapshot {
            assessment,
            fingerprint,
        }
    }

    pub fn assessment(&self) -> &Assessment {
        &self.assessment
    }

    pub fn fingerprint(&self) -> &ProtectionFingerprint {
        &self.fingerprint
    }
}

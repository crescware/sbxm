use super::{Assessment, ProtectionFingerprint};

/// `Assessment`と、その内容から作った`ProtectionFingerprint`を持つ、
/// ある瞬間の観測結果。
///
/// `new`だけがfingerprintを生成する。field は非公開とし、表示用のread-only
/// accessorだけを公開する。
///
/// `new`は`gate`だけが呼ぶ。commandが任意の`Assessment`からsnapshotを作れると、同じ
/// 観測結果から作った2つのsnapshotを`confirm`と`authorize`へ渡すだけで、状態を1回も
/// 観測せずにpermitを得られてしまう。
#[derive(Debug)]
pub struct ProtectionSnapshot {
    pub(super) assessment: Assessment,
    pub(super) fingerprint: ProtectionFingerprint,
}

impl ProtectionSnapshot {
    pub(super) fn new(assessment: Assessment) -> ProtectionSnapshot {
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

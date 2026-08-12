/// 共通ゲートだけが生成できるopaqueな許可証。
///
/// fieldはprivateとし、`gate::authorize`が拒否理由（`Blocker`）の不在を
/// 確認した場合だけ発行する。
#[derive(Debug)]
pub struct ProtectionPermit {
    _private: (),
}

impl ProtectionPermit {
    pub(super) fn issue() -> ProtectionPermit {
        ProtectionPermit { _private: () }
    }
}

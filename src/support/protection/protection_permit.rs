/// 共通ゲートだけが生成できるopaqueな許可証。
///
/// fieldはprivateとし、`gate::authorize`が層Aの通過を確認した場合だけ発行する。
#[derive(Debug)]
pub struct ProtectionPermit {
    _private: (),
}

impl ProtectionPermit {
    pub(super) fn issue() -> ProtectionPermit {
        ProtectionPermit { _private: () }
    }
}

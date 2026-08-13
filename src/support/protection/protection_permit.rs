use crate::project::SandboxName;

/// 共通ゲートだけが生成できるopaqueな許可証。
///
/// fieldはprivateとし、`gate::authorize`が拒否理由（`Blocker`）の不在と、確認した状態が
/// 変わっていないことを確かめた場合だけ発行する。操作種別は`authorize`が
/// confirmationと現在の観測の間で突き合わせ済みであり、ここでは持たない。
///
/// 許可した対象は自分で持つ。削除する側は名前を引数で受け取らず
/// [`ProtectionPermit::sandbox`]から取るため、「ある Sandbox への許可証で別の Sandbox を
/// 消す」経路が呼び出し規約ではなく型で無くなる。
#[derive(Debug)]
pub struct ProtectionPermit {
    sandbox: SandboxName,
}

impl ProtectionPermit {
    pub(super) fn issue(sandbox: SandboxName) -> ProtectionPermit {
        ProtectionPermit { sandbox }
    }

    /// 削除してよいと許可した Sandbox。
    pub fn sandbox(&self) -> &SandboxName {
        &self.sandbox
    }
}

/// rebuildとdestroyで、保護ゲートの判定が変わる点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructiveOperation {
    /// 管理外の作業ツリーは`ProtectionBlocker`で拒否する。rebuildは同じ配置を
    /// 再作成できない。
    Rebuild,
    /// 管理外の作業ツリーは内容を他の検査と同列に確かめたうえで、存在自体を
    /// `ConfirmableLoss`として明示確認の対象に載せる。
    Destroy,
}

impl DestructiveOperation {
    /// 翻訳しない安定した表記。fingerprintの入力に使う。
    pub fn as_str(self) -> &'static str {
        match self {
            DestructiveOperation::Rebuild => "rebuild",
            DestructiveOperation::Destroy => "destroy",
        }
    }
}

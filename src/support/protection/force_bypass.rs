/// `destroy --force`専用の、保護ゲートを意図的に迂回するopaqueな許可。
///
/// 通常経路の`ProtectionPermit`とは相互変換できない。同じboolean値で表現すると、
/// 迂回と通常許可を取り違えて渡せてしまう。
#[derive(Debug)]
pub struct ForceBypass {
    _private: (),
}

impl ForceBypass {
    /// `destroy --force`の分岐だけが呼ぶ、保護ゲートの意図的な迂回。
    ///
    /// architecture testが、他の呼び出し箇所が残っていないことを確認する。
    pub(crate) fn force_destroy() -> ForceBypass {
        ForceBypass { _private: () }
    }
}

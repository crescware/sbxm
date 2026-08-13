/// originを権威ある状態として使えない理由。
///
/// `observe_origin::observe_for_mutation`が使う語彙であり、呼び出し元ごとに別のenumを
/// 作らない。読み取り専用観測（`observe_read_only`）を追加するfollow-upは、この語彙へ
/// `ReadOnlyDataInsufficient`を足す形で拡張し、別のenumを作らない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnobservableReason {
    /// bare repositoryにoriginが設定されていない。
    OriginMissing,
    /// originのrefreshが完了しなかった。networkとcredentialの失敗を区別しない。
    RefreshFailed,
    /// originが返したref一覧を解釈できなかった。
    AdvertisementInvalid,
    /// 到達可能性の判定に必要なobjectがない。
    ObjectMissing,
}

impl UnobservableReason {
    /// fingerprintの入力に使う、翻訳しない安定表記。
    pub(super) fn fingerprint_key(self) -> &'static str {
        match self {
            UnobservableReason::OriginMissing => "origin-missing",
            UnobservableReason::RefreshFailed => "refresh-failed",
            UnobservableReason::AdvertisementInvalid => "advertisement-invalid",
            UnobservableReason::ObjectMissing => "object-missing",
        }
    }
}

/// originを権威ある状態として使えない理由。
///
/// `observe_origin::observe_for_mutation`が使う語彙であり、呼び出し元ごとに別のenumを
/// 作らない。読み取り専用観測（`observe_read_only`）もこの語彙を共有する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnobservableReason {
    /// bare repositoryにoriginが設定されていない。
    OriginMissing,
    /// originのrefreshが完了しなかった。networkとcredentialの失敗を区別しない。
    RefreshFailed,
    /// originが返したref一覧を解釈できなかった。
    AdvertisementInvalid,
    /// 到達可能性の判定に必要なobjectがない。
    ObjectMissing,
    /// fetchをしない読み取り専用観測だけでは、必要なremote objectを確かめられない。
    ReadOnlyDataInsufficient,
}

impl UnobservableReason {
    /// 表示とsnapshotへ使う、翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            UnobservableReason::OriginMissing => "origin-missing",
            UnobservableReason::RefreshFailed => "refresh-failed",
            UnobservableReason::AdvertisementInvalid => "advertisement-invalid",
            UnobservableReason::ObjectMissing => "object-missing",
            UnobservableReason::ReadOnlyDataInsufficient => "read-only-data-insufficient",
        }
    }

    /// fingerprintの入力に使う、翻訳しない安定表記。
    pub(super) fn fingerprint_key(self) -> &'static str {
        self.as_str()
    }
}

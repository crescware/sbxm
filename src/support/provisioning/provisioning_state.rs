/// 初回構築の現在状態。成果物から毎回読み取り、metadataへは保存しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisioningState {
    /// 登録直後で、再利用できる部分的成果物もない。
    Fresh,
    /// 目標構成の全post-conditionが観測できた。
    Ready,
    /// intentが残っている。成果物が完成していても、完了確認とintentのclearが未済。
    Pending,
    /// intentはないが、初回構築の成果物が部分的に残っている。
    Incomplete,
}

impl ProvisioningState {
    /// 翻訳しない安定した状態名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Ready => "ready",
            Self::Pending => "pending",
            Self::Incomplete => "incomplete",
        }
    }
}

impl std::fmt::Display for ProvisioningState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

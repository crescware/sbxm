use super::ProjectState;

/// registry entryを起点に観測した、1案件の現在の状態。
///
/// 状態名を永続化せず、entry、project root、metadata、Sandbox一覧という事実から
/// そのつど算出する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    /// project rootが無い。作成前に中断したか、あとから移動・削除された。
    Missing,
    /// project rootはあるが、metadataがまだ無い。
    Incomplete,
    /// metadataがregistry entryと一致しない、または読めない。
    Inconsistent,
    /// 一致するmetadataがあり、Sandboxの状態まで観測できた。
    Registered(ProjectState),
}

impl Observed {
    /// 翻訳しない安定した表記。
    pub fn as_str(&self) -> &'static str {
        match self {
            Observed::Missing => "missing",
            Observed::Incomplete => "incomplete",
            Observed::Inconsistent => "inconsistent",
            Observed::Registered(state) => state.as_str(),
        }
    }

    /// 登録が完了し、成果物がentryと一致しているか。
    pub fn is_settled(&self) -> bool {
        matches!(self, Observed::Registered(_))
    }
}

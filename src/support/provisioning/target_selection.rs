use crate::design::Warning;

use super::ObservedGeneration;

/// 初回構築を完成させる世代と、それを選ぶために行った観測。
///
/// `provision`はこれをconsumeすることでしか進めない。世代選択のためにhostを観測した
/// 結果を一緒に運ぶため、同じimageを見直す必要がない。
pub(crate) struct TargetSelection {
    pub(crate) generation: String,
    pub(crate) warnings: Vec<Warning>,
    /// 保存済み世代のimageを観測した結果。Dockerfileが変わっていなければ観測しない
    /// ため`None`になる。
    pub(super) stored: Option<ObservedGeneration>,
}

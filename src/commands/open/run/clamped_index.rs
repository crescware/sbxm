/// promptで確定したindexが、lock済みmetadataの範囲まで下げられたこと。
///
/// promptはmetadataを読む前に開くため、案件の実際のworktree数を超える値も確定できる。
/// 下げた事実は接続前に見せる。黙って別のworktreeへ繋ぐと、利用者が選んだ値と
/// 接続先が食い違ったまま作業が始まる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClampedIndex {
    /// 利用者がpromptで確定した値。
    pub requested: u32,
    /// metadataの範囲へ収めた、実際に開くindex。
    pub opened: u32,
}

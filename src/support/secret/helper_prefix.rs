/// sbxmが書くcredential helperの、placeholderより前の部分。
///
/// 値が変わってもsbxm自身が書いたものだと判別できるよう、固定部分を1箇所で持つ。
pub(super) const HELPER_PREFIX: &str = "!f() { echo username=x; echo password=";

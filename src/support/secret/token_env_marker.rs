/// sbxmが書いたtoken環境変数fileの1行目。
///
/// 値が変わっても、sbxm自身が書いたfileだと判別できるようにする。
pub(super) const TOKEN_ENV_MARKER: &str = "# Written by sbxm.";

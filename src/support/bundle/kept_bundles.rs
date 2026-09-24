/// 受け取ったbundleを、新しいものから何件残すか。
///
/// bundleは転送のための一時fileであり、取り込んだ内容はrepositoryのrefが持つ。直近の
/// 数件だけを、取り込みに失敗したときの手掛かりとして残す。
pub const KEPT_BUNDLES: usize = 3;

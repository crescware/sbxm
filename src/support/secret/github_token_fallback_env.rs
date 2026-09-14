/// `gh`が`GH_TOKEN`の次に読む環境変数名。
///
/// 組み込み`github` serviceはこの名前もsentinelで埋める。片方だけを上書きすると、
/// `GH_TOKEN`を読まないtoolがsentinelを送ることになる。
pub(super) const GITHUB_TOKEN_FALLBACK_ENV: &str = "GITHUB_TOKEN";

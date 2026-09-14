/// Sandboxの中で、GitHub tokenの環境変数を定めるfile。sbxmが持つ。
///
/// login shellは`/etc/profile`経由でこのdirectoryのscriptを読む。組み込みserviceが
/// processのenvironmentへ入れたsentinelを、shellの中で上書きするために使う。
/// `run-parts`は名前順に読むため、Docker Sandboxes自身が置く
/// `sandbox-persistent.sh`より後に読まれる名前にしてある。利用者が自分の値を書く
/// `/etc/sandbox-persistent.sh`とは別のfileであり、そちらには触れない。
pub(super) const TOKEN_ENV_FILE: &str = "/etc/profile.d/sbxm-github-token.sh";

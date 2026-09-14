use crate::boundary::host::protocol::CustomSecret;

use super::GITHUB_HOSTS;

/// この登録が、sbxmの求めるhostをすべて覆うか。
///
/// hostを分けて登録するとplaceholderも分かれる。gitとghが別のplaceholderを提示する
/// ことになり、片方は必ず素通しになる。1件で覆うことを求めるのはこのためである。
pub(super) fn covers_github_hosts(custom: &CustomSecret) -> bool {
    GITHUB_HOSTS
        .iter()
        .all(|host| custom.targets.iter().any(|target| target == host))
}

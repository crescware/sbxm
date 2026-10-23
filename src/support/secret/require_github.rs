use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use super::{GithubLookup, GithubRegistration, lookup_github};

/// `GitHubのcustom` secretが登録済みであることを確認し、その識別情報を返す。
///
/// 未登録、または1件に決まらなければ、`lookup_github`が作った診断で前提条件不足として
/// 停止する。
pub fn require_github(host: &dyn HostEnvironment, sandbox: &str) -> Result<GithubRegistration> {
    match lookup_github(host, sandbox)? {
        GithubLookup::Registered(registration) => Ok(registration),
        GithubLookup::Unregistered(error) | GithubLookup::Ambiguous(error) => Err(error),
    }
}

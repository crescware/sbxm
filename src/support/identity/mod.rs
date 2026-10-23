//! Git identity。
//!
//! 案件の名義は利用者が選ぶ。選ばれた値はglobal configへ既定として保存され、登録時に
//! 案件metadataへsnapshotされる。Sandbox内へ設定するのは、そのsnapshotだけである。
//!
//! hostの`git config --global`はこの選択の候補にしかならない。読めなくても案件を
//! 止めない。値を決めるのはhostではなく利用者である。
//!
//! Sandbox内に既に値があれば、記録と違っても利用者が選んだ値として残し、上書きも
//! 拒否もしない。書くのは未設定の場合だけである。
//!
//! `gh`のprotocol設定もここに置く。値の性質が同じで、既にある値の扱いを同じ形で
//! 決めるためである。呼ぶかどうかは`tools`が`gh`の有無で決める。

mod candidate_from_host;
mod ensure;
mod ensure_git_protocol;
mod git_protocol;
mod github_host;
mod observe;
mod observe_git_protocol;

pub use candidate_from_host::candidate_from_host;
pub use ensure::ensure;
pub(super) use ensure_git_protocol::ensure_git_protocol;
use git_protocol::GIT_PROTOCOL;
use github_host::GITHUB_HOST;
pub use observe::observe;
pub use observe_git_protocol::observe_git_protocol;

#[cfg(test)]
#[path = "identity_test.rs"]
mod identity_test;

//! Git identity。
//!
//! 案件の名義は利用者が選ぶ。選ばれた値はglobal configへ既定として保存され、登録時に
//! 案件metadataへsnapshotされる。Sandbox内へ設定するのは、そのsnapshotだけである。
//!
//! hostの`git config --global`はこの選択の候補にしかならない。読めなくても案件を
//! 止めない。値を決めるのはhostではなく利用者である。
//!
//! Sandbox内の既存の設定値が同じならそのままにし、異なる場合は別の利用者のSandboxで
//! ある可能性があるため自動で上書きしない。
//!
//! `gh`のprotocol設定もここに置く。値の性質が同じで、一致しない値の扱いを同じ形で
//! 決めるためである。呼ぶかどうかは`tools`が`gh`の有無で決める。

mod candidate_from_host;
mod ensure;
mod ensure_git_protocol;
mod git_protocol;
mod github_host;
mod mismatch;
#[allow(dead_code)]
mod verify;
#[allow(dead_code)]
mod verify_git_protocol;

pub use candidate_from_host::candidate_from_host;
pub use ensure::ensure;
pub(super) use ensure_git_protocol::ensure_git_protocol;
use git_protocol::GIT_PROTOCOL;
use github_host::GITHUB_HOST;
use mismatch::mismatch;
pub(crate) use verify::verify;
pub(crate) use verify_git_protocol::verify_git_protocol;

#[cfg(test)]
#[path = "identity_test.rs"]
mod identity_test;

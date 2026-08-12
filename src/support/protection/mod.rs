//! rebuild / destroyの全ての通常経路が通る、共通の破壊前保護ゲート。呼び出し側は
//! 個別の検査を選ばない。
//!
//! `gate::assess`が、worktreeの追跡対象変更・未追跡path、進行中のGit操作、管理外
//! worktree（rebuildのみ）、originから回収できると証明できないcommitを固定順序で
//! 観測し、[`Blocker`]へ集める。1件でもあれば`gate::authorize`が確認を求めず拒否
//! する。状態を観測できない場合も、安全と推測せず同様に拒否する。原因ごとの
//! `Blocker`と固有の`ErrorId`を持ち、共通のIDへ丸めない。観測不能の
//! diagnosticも、観測段階が付けたIDのまま保持する。
//!
//! `inspect`はgate配下だけが呼ぶprivate collectorであり、productionから直接公開
//! しない。`destroy --force`は保護を意図的に迂回する別操作であり、通常経路の
//! remediationには案内しない。
//!
//! commitを失わないが自動復元できない情報の明示確認や、削除を許可する証拠の型は、
//! 実際にそれを使うconsumerと同時に導入する。破壊前保護の方針そのもの（拒否と
//! 確認の二層構成、fail-closed、forceの扱い）は`docs/design-principles.md`が定める。

mod assessment;
mod blocker;
mod destructive_operation;
pub mod gate;
mod inspect;
mod kind;
mod mode;
mod origin_recovery_failure;
mod remote;
mod request;
mod worktree_report;

pub use assessment::Assessment;
pub use blocker::Blocker;
pub use destructive_operation::DestructiveOperation;
pub use kind::Kind;
pub use mode::Mode;
pub use origin_recovery_failure::OriginRecoveryFailure;
pub use remote::Remote;
pub use request::Request;
pub use worktree_report::WorktreeReport;

#[cfg(test)]
#[path = "protection_test.rs"]
mod protection_test;

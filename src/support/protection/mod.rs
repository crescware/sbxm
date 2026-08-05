//! rebuild / destroyの全ての通常経路が通る、共通の破壊前保護ゲート。
//!
//! `gate::assess`が層Aの観測を固定順序で行い、`gate::authorize`が層Aの通過だけを
//! 確認して`ProtectionPermit`を発行する。`inspect`はgate配下だけが呼ぶprivate
//! collectorであり、productionから直接公開しない。`force_bypass::force_destroy`は
//! `destroy --force`の分岐だけが使う、意図的な迂回である。

mod destructive_operation;
mod force_bypass;
pub mod gate;
mod inspect;
mod kind;
mod mode;
mod origin_recovery_failure;
mod protection_assessment;
mod protection_blocker;
mod protection_permit;
mod protection_request;
mod remote;
mod worktree_report;

pub use destructive_operation::DestructiveOperation;
pub use force_bypass::ForceBypass;
pub use kind::Kind;
pub use mode::Mode;
pub use origin_recovery_failure::OriginRecoveryFailure;
pub use protection_assessment::ProtectionAssessment;
pub use protection_blocker::ProtectionBlocker;
pub use protection_permit::ProtectionPermit;
pub use protection_request::ProtectionRequest;
pub use remote::Remote;
pub use worktree_report::WorktreeReport;

#[cfg(test)]
#[path = "protection_test.rs"]
mod protection_test;

//! rebuild / destroyの全ての通常経路が通る、共通の破壊前保護ゲート。呼び出し側は
//! 個別の検査を選ばない。
//!
//! 保護は回復可能性に基づく二層で構成する。
//!
//! - 層A（拒否）: 追跡対象ファイルの未commit変更、無視対象でない未追跡file、進行中の
//!   Git操作、管理外worktree（rebuildのみ）、originから回収できると証明できないcommit
//!   のいずれか1件でもあれば、確認を求めず削除しない。原因ごとに[`ProtectionBlocker`]の
//!   variantと固有の`ErrorId`を持ち、共通のIDへ丸めない。状態を観測できない場合も、
//!   安全と推測せず層Aと同様に拒否する。
//! - 層B（明示確認）: commit自体は失わないが自動復元できない情報（無視対象のpath、
//!   destroy対象の管理外worktreeの存在など）は、削除計画へ全件示し、対象sandbox名の
//!   完全一致入力を得た場合だけ削除を許可する。
//!
//! `gate::assess`が層Aの観測を固定順序で行い、`gate::authorize`が層Aの通過だけを
//! 確認する。`inspect`はgate配下だけが呼ぶprivate collectorであり、productionから
//! 直接公開しない。`destroy --force`は保護を意図的に迂回する別操作であり、通常経路の
//! remediationには案内しない。protected remove境界と、そこへ渡すopaqueな証拠は後続
//! Stepで実consumerと同時に導入する。

mod destructive_operation;
pub mod gate;
mod inspect;
mod kind;
mod mode;
mod origin_recovery_failure;
mod protection_assessment;
mod protection_blocker;
mod protection_request;
mod remote;
mod worktree_report;

pub use destructive_operation::DestructiveOperation;
pub use kind::Kind;
pub use mode::Mode;
pub use origin_recovery_failure::OriginRecoveryFailure;
pub use protection_assessment::ProtectionAssessment;
pub use protection_blocker::ProtectionBlocker;
pub use protection_request::ProtectionRequest;
pub use remote::Remote;
pub use worktree_report::WorktreeReport;

#[cfg(test)]
#[path = "protection_test.rs"]
mod protection_test;

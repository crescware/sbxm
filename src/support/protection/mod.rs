//! rebuild / destroyの全ての通常経路が通る、共通の破壊前保護ゲート。呼び出し側は
//! 個別の検査を選ばない。
//!
//! 保護は回復可能性に基づく2種類の結果で構成する。
//!
//! - [`Blocker`]（拒否理由）: 追跡対象ファイルの未commit変更、無視対象で
//!   ない未追跡file、進行中のGit操作、管理外worktree（rebuildのみ）、originから
//!   回収できると証明できないcommitのいずれか1件でもあれば、確認を求めず削除しない。
//!   原因ごとに固有の`ErrorId`を持ち、共通のIDへ丸めない。状態を観測できない場合も、
//!   安全と推測せずこれと同様に拒否する。
//! - [`ConfirmableLoss`]（確認すれば削除してよい対象）: 指すcommit自体は
//!   `Blocker`の検査でoriginから回収できると確認済みだが、削除後は自動復元
//!   できない付随情報（無視対象のpath、ref名、branchのupstream追跡、tag、追加remote名、
//!   reflogにだけ残るcommit、destroy対象の管理外worktree、sandboxの書き込み層）は、
//!   削除計画へ全件示し、対象sandbox名の完全一致入力を得た場合だけ削除を許可する。
//!
//! `gate::assess`がこの2種類の観測を固定順序で行い、[`ProtectionSnapshot`]（観測結果と
//! その正規化fingerprintの組）を作る。`Blocker`が1件も無い`ProtectionSnapshot`
//! だけを[`confirmation::confirm`]へ渡すと、sandbox名の完全一致を条件に
//! [`ProtectionConfirmation`]を得られる。`gate::authorize`は、そのconfirmationの
//! sandbox名・操作種別・fingerprintが**remove直前に取り直した**現在の`ProtectionSnapshot`
//! と完全一致し、かつ現在`Blocker`が1件も無い場合だけ[`ProtectionPermit`]を
//! 発行する。確認から実際の削除までのあいだに状態が変わればfingerprintが変わり、permitは
//! 発行されない。`ProtectionConfirmation`と`ProtectionPermit`はどちらも`Clone`/`Copy`に
//! せず、別run・別sandbox・別状態での使い回しを型で防ぐ。
//!
//! `inspect`はgate配下だけが呼ぶprivate collectorであり、productionから直接公開しない。
//! `destroy --force`は保護を意図的に迂回する別操作であり、通常経路のremediationには
//! 案内しない。

mod assessment;
mod blocker;
mod confirm_prompt;
mod confirmable_loss;
pub mod confirmation;
mod destructive_operation;
pub mod gate;
mod inspect;
mod kind;
mod mode;
mod origin_recovery_failure;
mod prompt_ui;
mod protection_confirmation;
mod protection_fingerprint;
mod protection_permit;
mod protection_snapshot;
mod remote;
mod request;
mod worktree_report;

pub use assessment::Assessment;
pub use blocker::Blocker;
pub use confirm_prompt::ConfirmPrompt;
pub use confirmable_loss::ConfirmableLoss;
pub use destructive_operation::DestructiveOperation;
pub use kind::Kind;
pub use mode::Mode;
pub use origin_recovery_failure::OriginRecoveryFailure;
pub use protection_confirmation::ProtectionConfirmation;
pub use protection_fingerprint::ProtectionFingerprint;
pub use protection_permit::ProtectionPermit;
pub use protection_snapshot::ProtectionSnapshot;
pub use remote::Remote;
pub use request::Request;
pub use worktree_report::WorktreeReport;

#[cfg(test)]
#[path = "protection_test.rs"]
mod protection_test;

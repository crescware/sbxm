//! `destroy`の出力。
//!
//! 破壊操作の確認画面ではgreenを使わない。削除対象の見出しもredではなくboldとし、
//! 保持対象は既定色にする。redを画面全体へ広げるとerrorと見分けにくくなり、消える
//! ものと残るものを落ち着いて比べられなくなる。
//!
//! force modeの通知と、計画のために停止中のSandboxを起動したという通知は、どちらも
//! 結果ではなく注意であるため、warningとしてstderrへ出す。

mod force_notice;
mod outcome_document;
mod plan_document;
mod started_notice;
mod target;

pub use force_notice::force_notice;
pub use outcome_document::outcome_document;
pub use plan_document::plan_document;
pub use started_notice::started_notice;
use target::target;

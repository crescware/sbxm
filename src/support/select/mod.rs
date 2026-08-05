//! 対象案件の選択。
//!
//! 案件の場所はglobal registryだけが持つ。引数で完全指定された場合もpromptで選んだ
//! 場合も、registryが指すproject rootのmetadataを読む。cwdや配置規則から対象を
//! 推測しない。
//!
//! 引数を省略できるcommandでは、registryが持つ案件を、既定選択のないpromptとして
//! stderrへ表示する。EscとCtrl-Cは何も変更せず終了する。
//!
//! 選択はSandboxの状態を読む前に終える。対象が決まる前にhostの状態へ触れない。

mod candidate;
mod candidates;
mod find;
mod incomplete_registration;
mod inconsistent_registration;
mod labels;
mod locked;
mod many;
mod no_managed_projects;
mod not_managed;
mod one;
mod open;
mod project_prompt;
mod prompt_ui;
mod unresolved;

pub use candidate::Candidate;
pub use candidates::candidates;
pub use find::find;
pub use incomplete_registration::incomplete_registration;
pub use inconsistent_registration::inconsistent_registration;
use labels::labels;
pub use locked::Locked;
pub use many::many;
use no_managed_projects::no_managed_projects;
pub use not_managed::not_managed;
pub use one::one;
pub use open::open;
pub use project_prompt::ProjectPrompt;
pub use unresolved::unresolved;

#[cfg(test)]
#[path = "select_test.rs"]
mod select_test;

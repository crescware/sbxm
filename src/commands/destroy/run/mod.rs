//! `sbxm destroy`。
//!
//! 対象Sandboxとsbxmの管理情報を破棄し、案件を`unmanaged`へ戻す。host cloneと
//! 利用者が管理する成果物は保持する。

mod confirm;
mod confirm_prompt;
mod destroy_outcome;
mod destroy_plan;
mod execute;
mod keeps;
mod prepare;
mod prepared;
mod prompt_ui;
mod re_register;
mod removes;
mod target;
mod unregister;
mod unregistration;

pub use confirm::confirm;
pub use confirm_prompt::ConfirmPrompt;
pub use destroy_outcome::DestroyOutcome;
pub use destroy_plan::DestroyPlan;
pub use execute::execute;
use keeps::keeps;
pub use prepare::prepare;
pub use prepared::Prepared;
use re_register::re_register;
use removes::removes;
pub use target::Target;
pub use unregister::unregister;
pub use unregistration::Unregistration;

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

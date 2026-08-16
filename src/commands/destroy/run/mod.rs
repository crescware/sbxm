//! `sbxm destroy`。
//!
//! 対象Sandboxとsbxmの管理情報を破棄し、案件を`unmanaged`へ戻す。host cloneと
//! 利用者が管理する成果物は保持する。

mod confirm;
mod destroy_outcome;
mod destroy_plan;
mod execute_bypassed;
mod execute_confirmed;
mod finish_removal;
mod keeps;
mod prepare;
mod prepared;
mod re_register;
mod removes;
mod target;
mod unregister;
mod unregistration;

pub use confirm::confirm;
pub use destroy_outcome::DestroyOutcome;
pub use destroy_plan::DestroyPlan;
pub use execute_bypassed::execute_bypassed;
pub use execute_confirmed::execute_confirmed;
use finish_removal::finish_removal;
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

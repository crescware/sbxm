//! 組み込みpresetの公開ID・出自を持つpolicy一覧。

mod allow_all;
mod balanced;
mod default_rule;
mod deny_all;

pub use allow_all::allow_all;
pub use balanced::balanced;
use default_rule::default_rule;
pub use deny_all::deny_all;

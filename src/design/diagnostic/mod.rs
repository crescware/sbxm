//! warningとremediationの構造。
//!
//! 「説明」と「実行するcommand」を一つの翻訳messageへ混ぜない。混ぜると、command行を
//! 独立させるという不変条件をrendererが守れなくなり、翻訳者がcommandの綴りを預かる
//! ことにもなる。説明は翻訳resource、commandはRust側のmodelが持つ。

mod remediation;
mod warning;

pub use remediation::Remediation;
pub use warning::Warning;

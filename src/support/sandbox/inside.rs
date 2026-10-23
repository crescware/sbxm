use crate::boundary::host::CommandOutcome;
use crate::diagnostics::Result;

/// Sandboxの中で走らせたcommandの結果の出所。
///
/// 観測は、どのcommandの結果をどう読むかだけを決め、結果が今起動したものか、まとめた
/// 確認が先に並べたものかを知らない。`Live`は起動し、`Recorded`は並べた結果から答える。
pub trait Inside {
    /// `sbx exec <sandbox> -- <args>`の結果。
    fn exec(&self, args: &[&str]) -> Result<CommandOutcome>;
}

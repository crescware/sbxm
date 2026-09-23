use crate::metadata::InitialProvisioningFile;

/// 既存のdestinationと内容が異なる場合の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict<'a> {
    /// 構築。構築の途中で利用者のfileを上書きしない。
    Refuse,
    /// 作り直したSandboxへの配置、または利用者が明示した上書き。
    Overwrite,
    /// `apply`。sbxmが最後に置いた内容のままのfileだけを置き換える。
    ///
    /// Sandbox側で書き換えられたfileを上書きすると、書き換えた内容は取り戻せない。
    /// baselineに記録が無いfileも、sbxmが置いたものかどうか分からないため置き換えない。
    Protect(&'a [InitialProvisioningFile]),
}

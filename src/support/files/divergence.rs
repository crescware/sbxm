/// Sandboxが、baselineと異なる内容を持っていたときの読み方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divergence {
    /// 初回構築の途中。baselineはこれから置く内容であり、異なる内容はsbxmが置いたもの
    /// ではない。どこから来たか分からないため、衝突として拒否する。
    Conflict,
    /// 完成後。baselineはsbxmが最後に置いた内容であり、異なる内容はそのあとSandboxの
    /// 中で書き換えられたものである。欠落でも衝突でもなく、変更として報告する。
    Modified,
}

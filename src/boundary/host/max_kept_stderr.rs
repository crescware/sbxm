/// 出力を流す実行で、診断のために溜めるstderrの上限byte数。
///
/// 流す実行はSandboxの出力のように信用しない相手を読む。stderrは診断にだけ使い、上限を
/// 超えた分は読んで捨てる。
pub(super) const MAX_KEPT_STDERR: usize = 64 * 1024;

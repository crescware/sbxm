/// 子processの出力の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputPolicy {
    /// stdoutとstderrを別々にbyte列としてcaptureする。
    ///
    /// 結果をparseする、または内容を秘匿するcommandへ使う。
    Capture,
    /// 人間向けの進捗を、外部toolが出したまま端末へ転送する。
    ///
    /// 長時間かかる工程の進捗を実行中に見せるために使う。sbxmは進捗の実況を重ねない。
    /// 何も出さない外部commandについては、工程の開始を`progress`が1行で予告する。
    /// captureしないため、失敗の診断にstderrの原文は含まれない。
    Passthrough,
    /// terminalそのものを引き渡す。
    ///
    /// SSH接続のように、利用者が入力する対話processへ使う。stdinも継承するため、
    /// 既存のterminal動作がそのまま保たれる。
    Inherit,
}

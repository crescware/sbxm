/// [`CommandLine::new`]が受け付けなかった理由。
///
/// 理由は呼び出し側が分岐するためだけに在る。[`CommandLine::optional`]が拒否された値を
/// blockごと省く以上、この理由が利用者へ届く経路は無い。届ける必要が生じた場合も、
/// 表示文字列はFTLから採るため、この型は表示用の文面を持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidCommandLine {
    /// 空の指示は「何を実行するか」を示さない。
    Empty,
    /// 改行を含む手順は一行のcommandではない。
    Multiline,
}

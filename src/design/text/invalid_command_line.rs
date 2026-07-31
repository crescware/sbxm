/// [`CommandLine::new`]が受け付けなかった理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidCommandLine {
    /// 空の指示は「何を実行するか」を示さない。
    Empty,
    /// 改行を含む手順は一行のcommandではない。
    Multiline,
}

impl std::fmt::Display for InvalidCommandLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            InvalidCommandLine::Empty => "a command line cannot be empty",
            InvalidCommandLine::Multiline => "a command line cannot span more than one line",
        })
    }
}

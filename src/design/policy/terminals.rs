/// streamごとのTTY判定と端末幅。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Terminals {
    pub stdout_is_tty: bool,
    pub stderr_is_tty: bool,
    /// 端末の桁数。読めない場合は`None`。
    pub width: Option<usize>,
}

impl Terminals {
    /// 実行中のprocessのstreamを観測する。
    pub fn detect() -> Terminals {
        use std::io::IsTerminal;
        let terminal = console::Term::stderr();
        Terminals {
            stdout_is_tty: std::io::stdout().is_terminal(),
            stderr_is_tty: std::io::stderr().is_terminal(),
            width: terminal.is_term().then(|| usize::from(terminal.size().1)),
        }
    }
}

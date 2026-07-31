//! 対話可能性。project省略時の規則へ使う。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interactivity {
    pub stdin_is_tty: bool,
    pub stderr_is_tty: bool,
}

impl Interactivity {
    /// 選択promptはstdinから読み、stderrへ表示する。両方がTTYでなければ使えない。
    pub fn can_prompt(self) -> bool {
        self.stdin_is_tty && self.stderr_is_tty
    }

    pub fn detect() -> Interactivity {
        use std::io::IsTerminal;
        Interactivity {
            stdin_is_tty: std::io::stdin().is_terminal(),
            stderr_is_tty: std::io::stderr().is_terminal(),
        }
    }
}

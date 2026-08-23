//! promptに使えるterminal能力の観測。

/// stdinとstderrの両方へpromptを出せるか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptCapability {
    available: bool,
}

impl PromptCapability {
    /// 実際のprocessのstreamからprompt能力を観測する。
    pub fn detect() -> Self {
        use std::io::IsTerminal;
        Self::from_streams(
            std::io::stdin().is_terminal(),
            std::io::stderr().is_terminal(),
        )
    }

    /// streamごとの観測結果からprompt能力を組み立てる。
    pub const fn from_streams(stdin_is_tty: bool, stderr_is_tty: bool) -> Self {
        Self::from_available(stdin_is_tty && stderr_is_tty)
    }

    /// testや既知の環境から能力を組み立てる。
    pub const fn from_available(available: bool) -> Self {
        Self { available }
    }

    pub const fn can_prompt(self) -> bool {
        self.available
    }
}

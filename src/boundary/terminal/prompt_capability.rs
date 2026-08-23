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
        Self::from_available(std::io::stdin().is_terminal() && std::io::stderr().is_terminal())
    }

    /// testや既知の環境から能力を組み立てる。
    pub const fn from_available(available: bool) -> Self {
        Self { available }
    }

    pub const fn can_prompt(self) -> bool {
        self.available
    }
}

use super::Tool;

/// Codex CLI。sbxmは何も行わない。
pub struct Codex;

impl Tool for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }
}

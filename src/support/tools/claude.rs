use super::Tool;

/// Claude Code。sbxmは何も行わない。
pub struct Claude;

impl Tool for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }
}

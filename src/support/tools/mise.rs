use super::Tool;

/// toolchain manager。sbxmは何も行わない。
///
/// managed worktreeが持ち込む設定をどう扱うかは、その案件の中の話である。sbxmはsandbox
/// を用意するところまでを持ち、中で何を動かすかは持たない。
pub struct Mise;

impl Tool for Mise {
    fn name(&self) -> &'static str {
        "mise"
    }
}

//! 既定のtemplateが入れる、sbxm自身の動作には要らないtool。
//!
//! Dockerfileは利用者の持ち物であり、この4つはどれも削れる。何が入っているかは
//! metadataに残らないため、Sandboxを観測して決める。
//!
//! toolは「何が起きたら自分は何をするか」だけを宣言する。何もしないtoolは既定の
//! noopのままにする。eventを上げる側はtoolを名指しせず、`ALL`を順に回す。
//!
//! eventはcommandではなく、起きたことで切る。`prepare`と`rebuild`はどちらもSandboxを
//! 使える状態にするため、同じeventを上げる。

mod all;
mod claude;
mod codex;
mod gh;
mod installed;
mod mise;
mod note;
mod probe;
mod sandbox_ready;
mod tool;
mod worktrees_ready;

pub use all::ALL;
pub use claude::Claude;
pub use codex::Codex;
pub use gh::Gh;
pub use installed::Installed;
pub use mise::Mise;
pub use note::Note;
pub use probe::probe;
pub use sandbox_ready::SandboxReady;
pub use tool::Tool;
pub use worktrees_ready::WorktreesReady;

#[cfg(test)]
#[path = "tools_test.rs"]
mod tools_test;

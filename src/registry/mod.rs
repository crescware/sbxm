//! Global registry `~/.sbxm/registry.yaml`。
//!
//! 任意の場所へ配置された全案件を、単一のdocumentで索引する。registryが持つのは索引と
//! 不変なrepository identityだけであり、可変な目標構成やGit identityはproject rootの
//! `.sbxm/project.yaml`だけを正本とする。
//!
//! 登録状態を表すfieldを保存しない。登録がどこまで進んでいるかは、entry、project root、
//! metadata、host cloneというfilesystem上の事実を観測して算出する。
//!
//! mutationは`~/.sbxm/registry.lock`に対するglobal exclusive lockで直列化し、documentを
//! 単純追記しない。全entryを検証し、memory上で完全なdocumentを組み立ててからatomicに
//! 置き換える。一部entryだけが正常でも、壊れたregistryをmutationの根拠として信用しない。

mod document;
mod document_version;
mod index;
mod load;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod parse;
mod registry_entry;
mod registry_guard;
mod registry_version;
mod render;

use document_version::DOCUMENT_VERSION;
pub use index::Index;
pub use load::load;
pub use parse::parse;
pub use registry_entry::RegistryEntry;
pub use registry_guard::RegistryGuard;
pub use registry_version::REGISTRY_VERSION;
pub use render::render;

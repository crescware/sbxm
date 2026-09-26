//! Sandboxとhostのあいだのcommitの受け渡し。
//!
//! `.git`を丸ごと写さない。hookやconfigが混ざり、git操作の途中で写したものは整合しない
//! ことがある。Sandboxからhostへは、hostのgitがssh越しにSandboxのrepositoryをfetchし、
//! objectを確かめてからhostのrepositoryのsbxm専用の名前空間へだけ取り込む。hostにある
//! repositoryを登録した案件では、逆向きにhostのbranchとtagをbundleにしてSandboxの
//! originとして置く。
mod auto_save;
mod auto_saved;
mod carries;
mod clear_save_refs;
mod finish_save;
mod import_from_sandbox;
mod place_bundle;
mod place_save_refs;
mod prepare_save;
mod ref_change;
mod ref_kinds;
mod reflect_result;
mod reflect_saved;
mod reflected;
mod require_something_to_send;
mod restore_saved_branches;
mod save_failed;
mod save_to_host;
mod saved_namespace;
mod saved_refs;
mod saved_tip;
mod saved_tips;
mod send_to_sandbox;
mod stamp;

pub use auto_save::auto_save;
pub use auto_saved::AutoSaved;
pub use carries::carries;
use clear_save_refs::CLEAR_SAVE_REFS;
use finish_save::finish_save;
pub use import_from_sandbox::import_from_sandbox;
use place_bundle::PLACE_BUNDLE;
pub(crate) use place_save_refs::PLACE_SAVE_REFS;
use prepare_save::prepare_save;
pub use ref_change::RefChange;
use ref_kinds::REF_KINDS;
pub use reflect_result::ReflectResult;
pub use reflect_saved::reflect_saved;
pub use reflected::Reflected;
pub use require_something_to_send::require_something_to_send;
pub use restore_saved_branches::restore_saved_branches;
pub use save_failed::save_failed;
pub use save_to_host::save_to_host;
pub use saved_namespace::saved_namespace;
use saved_refs::saved_refs;
pub use saved_tip::SavedTip;
pub use saved_tips::saved_tips;
pub use send_to_sandbox::send_to_sandbox;
pub use stamp::stamp;

#[cfg(test)]
#[path = "bundle_test.rs"]
mod bundle_test;

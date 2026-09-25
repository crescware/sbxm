//! git bundleによるSandboxとhostのあいだのcommitの受け渡し。
//!
//! `.git`を丸ごと写さない。hookやconfigが混ざり、git操作の途中で写したものは整合しない
//! ことがある。bundleが運ぶのはobjectとrefだけである。受け取ったbundleは検証してから、
//! hostのrepositoryのsbxm専用の名前空間へだけ取り込む。hostにあるrepositoryを登録した
//! 案件では、逆向きにhostのbranchとtagをbundleにしてSandboxのoriginとして置く。

mod auto_save;
mod auto_saved;
mod carries;
mod create_bundle;
mod import_bundle;
mod kept_bundles;
mod max_bundle_bytes;
mod place_bundle;
mod prune_bundles;
mod receipt;
mod receive_bundle;
mod received_bundle;
mod ref_change;
mod ref_kinds;
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
use create_bundle::CREATE_BUNDLE;
pub use import_bundle::import_bundle;
pub use kept_bundles::KEPT_BUNDLES;
pub use max_bundle_bytes::MAX_BUNDLE_BYTES;
use place_bundle::PLACE_BUNDLE;
pub use prune_bundles::prune_bundles;
pub use receipt::Receipt;
pub use receive_bundle::receive_bundle;
pub use received_bundle::ReceivedBundle;
pub use ref_change::RefChange;
use ref_kinds::REF_KINDS;
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

//! git bundleによるSandboxからhostへのcommitの受け取り。
//!
//! `.git`を丸ごと写さない。hookやconfigが混ざり、git操作の途中で写したものは整合しない
//! ことがある。bundleが運ぶのはobjectとrefだけである。受け取ったbundleは検証してから、
//! hostのrepositoryのsbxm専用の名前空間へだけ取り込む。

mod create_bundle;
mod import_bundle;
mod kept_bundles;
mod max_bundle_bytes;
mod prune_bundles;
mod receipt;
mod receive_bundle;
mod received_bundle;
mod ref_change;
mod ref_kinds;
mod save_to_host;
mod saved_namespace;
mod saved_on_host;
mod saved_refs;
mod saved_tip;
mod saved_tips;
mod stamp;

use create_bundle::CREATE_BUNDLE;
pub use import_bundle::import_bundle;
pub use kept_bundles::KEPT_BUNDLES;
pub use max_bundle_bytes::MAX_BUNDLE_BYTES;
pub use prune_bundles::prune_bundles;
pub use receipt::Receipt;
pub use receive_bundle::receive_bundle;
pub use received_bundle::ReceivedBundle;
pub use ref_change::RefChange;
use ref_kinds::REF_KINDS;
pub use save_to_host::save_to_host;
pub use saved_namespace::saved_namespace;
pub use saved_on_host::saved_on_host;
pub use saved_tip::SavedTip;
pub use saved_tips::saved_tips;
use saved_refs::saved_refs;
pub use stamp::stamp;

#[cfg(test)]
#[path = "bundle_test.rs"]
mod bundle_test;

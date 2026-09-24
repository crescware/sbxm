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
mod receive_bundle;
mod received_bundle;
mod ref_change;
mod stamp;

use create_bundle::CREATE_BUNDLE;
pub use import_bundle::import_bundle;
pub use kept_bundles::KEPT_BUNDLES;
pub use max_bundle_bytes::MAX_BUNDLE_BYTES;
pub use prune_bundles::prune_bundles;
pub use receive_bundle::receive_bundle;
pub use received_bundle::ReceivedBundle;
pub use ref_change::RefChange;
pub use stamp::stamp;

#[cfg(test)]
#[path = "bundle_test.rs"]
mod bundle_test;

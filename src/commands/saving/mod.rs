//! 破壊操作の前に、Sandboxのcommitをhostのrepositoryへ保存する手順。
//!
//! 保存そのものは`sbxm fetch`と同じ処理である。rebuildとdestroyは、保存で解ける理由
//! だけで断られたときに、ここを通して保存を申し出てから準備をやり直す。

mod offer_save;
mod prepare_offering_save;

use offer_save::offer_save;
pub use prepare_offering_save::prepare_offering_save;

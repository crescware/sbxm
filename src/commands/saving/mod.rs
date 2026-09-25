//! Sandboxのcommitを、commandの途中でhostのrepositoryへ保存する手順。
//!
//! 保存そのものは`sbxm fetch`と同じ処理である。hostにあるrepositoryの案件は、止める、
//! 作り直す、消す前と、sessionの間と後に、ここを通して自動で保存する。rebuildと
//! destroyは、保存で解ける理由だけで断られたときに、保存を申し出てから準備をやり直す。

mod auto_saved;
mod offer_save;
mod prepare_offering_save;
mod save_first;

pub use auto_saved::auto_saved;
use offer_save::offer_save;
pub use prepare_offering_save::prepare_offering_save;
pub use save_first::save_first;

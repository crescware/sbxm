//! Sandboxのcommitを、commandの途中でhostのrepositoryへ保存する手順。
//!
//! 保存は、Sandboxのcommitをhostのrepositoryの`refs/sbx/<sandbox>/`へ取り込み、hostの
//! branchとtagは動かさない。hostにあるrepositoryの案件は、止める、作り直す、消す前と、
//! sessionの間と後に、ここを通して自動で保存する。rebuildとdestroyは、保存で解ける
//! 理由だけで断られたときに、保存を申し出てから準備をやり直す。

mod auto_saved;
mod offer_save;
mod prepare_offering_save;
mod save_first;
mod save_now;
mod save_output;
mod save_selected;
mod saved_document;

pub use auto_saved::auto_saved;
use offer_save::offer_save;
pub use prepare_offering_save::prepare_offering_save;
pub use save_first::save_first;
use save_now::save_now;
use save_output::SaveOutput;
pub use save_selected::save_selected;
use saved_document::saved_document;

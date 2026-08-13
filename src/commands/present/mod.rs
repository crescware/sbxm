//! domain値を表示語彙へ写す。
//!
//! domain enumはUIの色を持たない。同じ`stopped`でも、停止commandの完了結果ならpositive、
//! 稼働要件のstatusならattentionである。文脈を知っているのはcommandであり、enumではない。
//! そのため`value -> color`のglobalな表を作らず、この層で`VisualState`を明示する。
//!
//! 凡例も同じ理由でここに置く。状態値は翻訳しないため、正本locale以外の正常出力には
//! 出現した値の説明を添える。表へ値を置くたびに凡例へ控えられるよう、写像と登録を
//! 1回の呼び出しにまとめる。

mod creation_mode;
mod global_status;
mod legend;
mod observed;
mod placement;
mod project_state;
mod project_status;
mod sandbox_state;
mod stop_result;
mod workspace_state;

pub use creation_mode::creation_mode;
pub use global_status::global_status;
pub use legend::Legend;
pub use observed::observed;
pub use placement::placement;
pub use project_state::project_state;
pub use project_status::project_status;
pub use sandbox_state::sandbox_state;
pub use stop_result::stop_result;
pub use workspace_state::workspace_state;

#[cfg(test)]
#[path = "present_test.rs"]
mod present_test;

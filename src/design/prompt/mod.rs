//! 選択promptと、自由入力。
//!
//! promptは「候補」「現在位置」「選択済み」を別の状態として描く。cursor一文字だけに
//! 識別を委ねると、色を消したときと画面外へ選択が出たときに状態が読めなくなる。
//!
//! 候補が多い一覧は、文字を打って絞り込める。一致は表示labelの連続した部分文字列であり、
//! 一致しない候補を隠すだけで並び順は変えない。現在位置と選択済みは行ではなく候補に付く
//! ため、絞り込みで行が隠れても失わない。絞り込めるかどうかは[`Filter`]で呼び出し側が
//! 決め、promptは候補の数から推測しない。
//!
//! 描画と打鍵の解釈を[`Selection`]と端末adapterへ分ける。状態遷移は端末なしで確かめられ、
//! 端末adapterは打鍵の供給と描画の操作だけを持つ。
//!
//! 端末は[`Keys`]と[`Screen`]として受け取る。実端末のadapterは`boundary::terminal`にあり、
//! [`PromptUi`]が決める進み方はfakeで確かめられる。
//!
//! prompt libraryの既定themeへ要件を合わせず、単一選択と複数選択で同じ描画を使う。
//! 自由入力とyes/noも同じ入口から出すのは、commandごとにpromptの見た目が変わらない
//! ようにするためである。
//!
//! [`PromptUi`]はlocaleと描画条件を写しで持つ。`Ui`とは別に組み立てて渡すため、同じ
//! workflowが進捗の報告とpromptの両方を必要としても借用が衝突しない。

mod action;
mod action_for;
#[cfg(test)]
mod fake;
mod filter;
mod key;
mod keys;
mod narrowing;
mod open_frame_fixed_rows;
mod open_selection;
mod open_viewport;
mod painter;
mod prompt_ui;
mod refusal;
mod screen;
mod selection;
mod transition;
mod unreadable;
mod viewport;
mod window;

pub use action::Action;
pub use action_for::action_for;
#[cfg(test)]
pub use fake::{RecordedScreen, ScriptedKeys};
pub use filter::Filter;
pub use key::Key;
pub use keys::Keys;
pub use narrowing::Narrowing;
use open_frame_fixed_rows::OPEN_FRAME_FIXED_ROWS;
pub use open_selection::OpenSelection;
use open_viewport::open_viewport;
use painter::Painter;
pub use prompt_ui::PromptUi;
use refusal::Refusal;
pub use screen::Screen;
pub use selection::Selection;
pub use transition::Transition;
pub use unreadable::unreadable;
use viewport::viewport;
pub use window::window;

#[cfg(test)]
#[path = "prompt_test.rs"]
mod prompt_test;

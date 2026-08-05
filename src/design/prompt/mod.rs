//! 選択promptと、自由入力。
//!
//! promptは「候補」「現在位置」「選択済み」を別の状態として描く。cursor一文字だけに
//! 識別を委ねると、色を消したときと画面外へ選択が出たときに状態が読めなくなる。
//!
//! 描画と打鍵の解釈を[`Selection`]と端末adapterへ分ける。状態遷移は端末なしで確かめられ、
//! 端末adapterは打鍵の供給と描画の操作だけを持つ。
//!
//! 端末は[`Keys`]と[`Screen`]として受け取る。実端末の操作を担うのは、その2つを満たす
//! [`RealTerminal`] adapterである。[`PromptUi`]が決める進み方はfakeで確かめられる。
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
mod index_selection;
mod keys;
mod painter;
mod prompt_ui;
mod real_terminal;
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
pub use index_selection::IndexSelection;
pub use keys::Keys;
use painter::Painter;
pub use prompt_ui::PromptUi;
use real_terminal::RealTerminal;
pub use screen::Screen;
pub use selection::Selection;
pub use transition::Transition;
pub use unreadable::unreadable;
use viewport::viewport;
pub use window::window;

#[cfg(test)]
#[path = "prompt_test.rs"]
mod prompt_test;

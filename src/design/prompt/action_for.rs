use console::Key;

use super::Action;

/// 打鍵から操作を決める。
///
/// 行編集は提供しない。選択に必要な打鍵だけを受け取る。
pub fn action_for(key: &Key) -> Action {
    match key {
        Key::ArrowUp => Action::Previous,
        Key::ArrowDown => Action::Next,
        Key::Char(' ') => Action::Toggle,
        Key::Enter => Action::Confirm,
        Key::Escape | Key::CtrlC => Action::Cancel,
        _ => Action::Ignore,
    }
}

#[cfg(test)]
#[path = "action_for_test.rs"]
mod action_for_test;

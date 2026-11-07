use super::{Action, Key};

/// 打鍵から操作を決める。
///
/// 行編集は提供しない。選択と絞り込みに必要な打鍵だけを受け取る。文字キーを絞り込みへ
/// 使うかどうかは、この対応ではなく状態機械が`Filter`で決める。
pub fn action_for(key: Key) -> Action {
    match key {
        Key::ArrowUp => Action::Previous,
        Key::ArrowDown => Action::Next,
        Key::ArrowLeft => Action::DecreaseIndex,
        Key::ArrowRight => Action::IncreaseIndex,
        Key::Char(' ') => Action::Toggle,
        Key::Enter => Action::Confirm,
        Key::Escape | Key::CtrlC => Action::Cancel,
        // 空白は選択に使い、制御文字は端末が別の意味を持たせている。どちらも絞り込み
        // 文字列には入れない。
        Key::Char(character) if !character.is_control() && !character.is_whitespace() => {
            Action::Type(character)
        }
        Key::Backspace => Action::Erase,
        _ => Action::Ignore,
    }
}

#[cfg(test)]
#[path = "action_for_test.rs"]
mod action_for_test;

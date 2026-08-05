use console::Key;

use super::{Action, action_for};

#[test]
fn every_accepted_keystroke_names_the_operation_it_performs() {
    // 打鍵と操作の対応そのものが仕様である。受け取る打鍵と、受け取らない打鍵を
    // 同じ表で示す。
    let expected = [
        (Key::ArrowUp, Action::Previous),
        (Key::ArrowDown, Action::Next),
        (Key::ArrowLeft, Action::DecreaseIndex),
        (Key::ArrowRight, Action::IncreaseIndex),
        (Key::Char(' '), Action::Toggle),
        (Key::Enter, Action::Confirm),
        (Key::Escape, Action::Cancel),
        (Key::CtrlC, Action::Cancel),
        // 行編集は提供しない。移動とindex変更と確定と取り消し以外は状態を変えない。
        (Key::Backspace, Action::Ignore),
        (Key::Tab, Action::Ignore),
        (Key::Char('x'), Action::Ignore),
        (Key::Unknown, Action::Ignore),
    ];
    for (key, action) in expected {
        assert_eq!(action_for(&key), action, "{key:?}");
    }
}

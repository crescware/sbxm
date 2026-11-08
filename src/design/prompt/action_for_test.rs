use super::{Action, Key, action_for};

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
        // 文字は絞り込みへ、Backspaceはその取り消しへ使う。
        (Key::Char('x'), Action::Type('x')),
        (Key::Char('/'), Action::Type('/')),
        (Key::Char('案'), Action::Type('案')),
        (Key::Backspace, Action::Erase),
        // 行編集は提供しない。制御文字と空白の類は絞り込み文字列にも入れない。
        (Key::Char('\u{15}'), Action::Ignore),
        (Key::Char('\u{a0}'), Action::Ignore),
        (Key::Tab, Action::Ignore),
        (Key::Home, Action::Ignore),
        (Key::Unknown, Action::Ignore),
    ];
    for (key, action) in expected {
        assert_eq!(action_for(key), action, "{key:?}");
    }
}

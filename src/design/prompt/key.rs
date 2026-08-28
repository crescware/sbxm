/// promptが扱う打鍵。
///
/// terminal libraryのkey型は`boundary::terminal`でこのportへ変換する。promptの状態遷移
/// とfakeは、具体的なterminal libraryなしで確かめられる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Unknown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Enter,
    Escape,
    Backspace,
    Home,
    Tab,
    Char(char),
    CtrlC,
}

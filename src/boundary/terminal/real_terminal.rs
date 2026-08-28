use console::{Key as ConsoleKey, Term};

use crate::design::prompt::{Key, Keys, Screen};

/// consoleの実端末をpromptのportへ接続するadapter。
pub(super) struct RealTerminal {
    term: Term,
}

impl RealTerminal {
    pub(super) fn new(term: Term) -> RealTerminal {
        RealTerminal { term }
    }
}

impl Keys for RealTerminal {
    fn read_key(&mut self) -> std::io::Result<Key> {
        self.term.read_key().map(|key| map_key(&key))
    }
}

impl Screen for RealTerminal {
    fn write_str(&mut self, text: &str) -> std::io::Result<()> {
        self.term.write_str(text)
    }

    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.term.write_line(line)
    }

    fn clear_chars(&mut self, count: usize) -> std::io::Result<()> {
        self.term.clear_chars(count)
    }

    fn clear_last_lines(&mut self, count: usize) -> std::io::Result<()> {
        self.term.clear_last_lines(count)
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        self.term.hide_cursor()
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        self.term.show_cursor()
    }

    /// 端末の高さ。端末でない場合は、libraryの既定値を観測値として扱わない。
    fn rows(&self) -> Option<u16> {
        self.term.is_term().then(|| self.term.size().0)
    }
}

fn map_key(key: &ConsoleKey) -> Key {
    match key {
        ConsoleKey::ArrowLeft => Key::ArrowLeft,
        ConsoleKey::ArrowRight => Key::ArrowRight,
        ConsoleKey::ArrowUp => Key::ArrowUp,
        ConsoleKey::ArrowDown => Key::ArrowDown,
        ConsoleKey::Enter => Key::Enter,
        ConsoleKey::Escape => Key::Escape,
        ConsoleKey::Backspace => Key::Backspace,
        ConsoleKey::Home => Key::Home,
        ConsoleKey::Tab => Key::Tab,
        ConsoleKey::Char(character) => Key::Char(*character),
        ConsoleKey::CtrlC => Key::CtrlC,
        ConsoleKey::Unknown
        | ConsoleKey::UnknownEscSeq(_)
        | ConsoleKey::End
        | ConsoleKey::BackTab
        | ConsoleKey::Alt
        | ConsoleKey::Del
        | ConsoleKey::Shift
        | ConsoleKey::Insert
        | ConsoleKey::PageUp
        | ConsoleKey::PageDown
        | _ => Key::Unknown,
    }
}

#[cfg(test)]
#[path = "real_terminal_test.rs"]
mod real_terminal_test;

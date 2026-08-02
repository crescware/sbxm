use std::cell::RefCell;
use std::rc::Rc;

use crate::design::width::display_width;

use super::super::Screen;

/// 書かれた内容を行として保つscreen。
///
/// 消す操作も実際に落とす。描き終えた画面に何が残るかをtestが読めるようにするため、
/// 最後まで残った行と、途中で描いた行の両方を持つ。
///
/// 写しを渡してもcommandが書いた結果を読めるよう、行は共有する。
#[derive(Clone)]
pub struct RecordedScreen {
    /// 画面に残っている行。末尾は書きかけの行とする。
    lines: Rc<RefCell<Vec<String>>>,
    /// 消された行も含む、書かれた順の全行。
    drawn: Rc<RefCell<Vec<String>>>,
    cursor: Rc<RefCell<bool>>,
    rows: Option<u16>,
    failure: Option<std::io::ErrorKind>,
}

impl RecordedScreen {
    /// 高さを読めない画面。一覧は制限されない。
    pub fn new() -> RecordedScreen {
        RecordedScreen {
            lines: Rc::new(RefCell::new(vec![String::new()])),
            drawn: Rc::new(RefCell::new(Vec::new())),
            cursor: Rc::new(RefCell::new(true)),
            rows: None,
            failure: None,
        }
    }

    /// 高さの決まった画面。
    pub fn with_rows(rows: u16) -> RecordedScreen {
        RecordedScreen {
            rows: Some(rows),
            ..RecordedScreen::new()
        }
    }

    /// 書けない画面。
    pub fn failing(kind: std::io::ErrorKind) -> RecordedScreen {
        RecordedScreen {
            failure: Some(kind),
            ..RecordedScreen::new()
        }
    }

    /// 描き終えた時点で画面に残っている行。
    pub fn lines(&self) -> Vec<String> {
        let mut lines = self.lines.borrow().clone();
        // 書きかけの行が空なら、それは行ではなくcursorの位置である。
        if lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines
    }

    /// 消された行も含めて、書かれた順の全行。
    pub fn drawn(&self) -> Vec<String> {
        self.drawn.borrow().clone()
    }

    pub fn cursor_is_visible(&self) -> bool {
        *self.cursor.borrow()
    }

    fn writable(&self) -> std::io::Result<()> {
        match self.failure {
            Some(kind) => Err(std::io::Error::from(kind)),
            None => Ok(()),
        }
    }

    /// 書きかけの行を書き換える。
    fn current(&self, edit: impl FnOnce(&mut String)) {
        let mut lines = self.lines.borrow_mut();
        if lines.is_empty() {
            lines.push(String::new());
        }
        if let Some(current) = lines.last_mut() {
            edit(current);
        }
    }
}

impl Screen for RecordedScreen {
    fn write_str(&mut self, text: &str) -> std::io::Result<()> {
        self.writable()?;
        self.current(|current| current.push_str(text));
        Ok(())
    }

    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.writable()?;
        self.current(|current| current.push_str(line));
        let mut lines = self.lines.borrow_mut();
        if let Some(completed) = lines.last() {
            self.drawn.borrow_mut().push(completed.clone());
        }
        lines.push(String::new());
        Ok(())
    }

    fn clear_chars(&mut self, count: usize) -> std::io::Result<()> {
        self.writable()?;
        self.current(|current| {
            let mut cleared = 0usize;
            while cleared < count {
                let Some(character) = current.pop() else {
                    break;
                };
                cleared = cleared.saturating_add(display_width(&character.to_string()));
            }
        });
        Ok(())
    }

    fn clear_last_lines(&mut self, count: usize) -> std::io::Result<()> {
        self.writable()?;
        let mut lines = self.lines.borrow_mut();
        // 書きかけの行はcursorの居場所であり、消す対象はその前にある。
        let current = lines.len().saturating_sub(1);
        lines.drain(current.saturating_sub(count)..current);
        Ok(())
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        self.writable()?;
        *self.cursor.borrow_mut() = false;
        Ok(())
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        self.writable()?;
        *self.cursor.borrow_mut() = true;
        Ok(())
    }

    fn rows(&self) -> Option<u16> {
        self.rows
    }
}

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Write};
use std::rc::Rc;

use super::{End, Event};

/// 書けた量をtestが決める書き込み端。書けたbyteを残す。
///
/// `answering`の答えが尽きた後は、何も受け取らない（`Ok(0)`）。`ScriptedOs`が子のstdinとして
/// 渡したものは、書けた量と閉じたことを、その記録へ残す。
pub struct ScriptedWriter {
    answers: Option<VecDeque<io::Result<usize>>>,
    written: Rc<RefCell<Vec<u8>>>,
    log: Option<Rc<RefCell<Vec<Event>>>>,
}

impl ScriptedWriter {
    /// 渡されたbyteをすべて受け取る書き込み端。
    pub fn accepting_all() -> ScriptedWriter {
        ScriptedWriter {
            answers: None,
            written: Rc::default(),
            log: None,
        }
    }

    /// 書き込みごとに、受け取るbyte数か失敗を順に答える書き込み端。
    pub fn answering(answers: impl IntoIterator<Item = io::Result<usize>>) -> ScriptedWriter {
        ScriptedWriter {
            answers: Some(answers.into_iter().collect()),
            written: Rc::default(),
            log: None,
        }
    }

    /// 受け取ったbyte。
    pub fn written(&self) -> Rc<RefCell<Vec<u8>>> {
        Rc::clone(&self.written)
    }

    /// 子のstdinとして、書けた量と閉じたことを`events`へ残す。
    pub(super) fn logged(mut self, events: &Rc<RefCell<Vec<Event>>>) -> ScriptedWriter {
        self.log = Some(Rc::clone(events));
        self
    }

    fn record(&self, event: Event) {
        if let Some(events) = &self.log {
            events.borrow_mut().push(event);
        }
    }
}

impl Write for ScriptedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let accepted = match self.answers.as_mut() {
            None => bytes.len(),
            Some(answers) => answers.pop_front().unwrap_or(Ok(0))?.min(bytes.len()),
        };
        self.written
            .borrow_mut()
            .extend_from_slice(&bytes[..accepted]);
        self.record(Event::Wrote(End::Stdin, accepted));
        Ok(accepted)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for ScriptedWriter {
    fn drop(&mut self) {
        self.record(Event::Closed(End::Stdin));
    }
}

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Read};
use std::rc::Rc;

use super::{End, Event, ReadStep};

/// 読みの結果をtestが決めるstream。
///
/// 台本が尽きた後は、書き手がすべて閉じたものとしてEOFを返す。`held_open`で作れば、子孫が
/// 書き込み端を持ち続けるものとして、いつまでも`WouldBlock`を返す。`ScriptedOs`が子の端として
/// 渡したものは、読むたびと閉じたときに、その記録へ残す。
pub struct ScriptedPipe {
    steps: VecDeque<ReadStep>,
    held_open: bool,
    log: Option<(End, Rc<RefCell<Vec<Event>>>)>,
}

impl ScriptedPipe {
    pub fn new(steps: impl IntoIterator<Item = ReadStep>) -> ScriptedPipe {
        ScriptedPipe {
            steps: steps.into_iter().collect(),
            held_open: false,
            log: None,
        }
    }

    /// 台本が尽きても閉じないstream。
    pub fn held_open(steps: impl IntoIterator<Item = ReadStep>) -> ScriptedPipe {
        ScriptedPipe {
            steps: steps.into_iter().collect(),
            held_open: true,
            log: None,
        }
    }

    /// 子の`end`として、読みと閉じたことを`events`へ残す。
    pub(super) fn logged(mut self, end: End, events: &Rc<RefCell<Vec<Event>>>) -> ScriptedPipe {
        self.log = Some((end, Rc::clone(events)));
        self
    }

    fn record(&self, event: impl FnOnce(End) -> Event) {
        if let Some((end, events)) = &self.log {
            events.borrow_mut().push(event(*end));
        }
    }
}

impl Read for ScriptedPipe {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.record(Event::Read);
        match self.steps.pop_front() {
            Some(ReadStep::Bytes(bytes)) => {
                let size = bytes.len().min(buffer.len());
                buffer[..size].copy_from_slice(&bytes[..size]);
                if size < bytes.len() {
                    self.steps.push_front(ReadStep::Bytes(&bytes[size..]));
                }
                Ok(size)
            }
            Some(ReadStep::Owned(mut bytes)) => {
                let size = bytes.len().min(buffer.len());
                buffer[..size].copy_from_slice(&bytes[..size]);
                if size < bytes.len() {
                    bytes.drain(..size);
                    self.steps.push_front(ReadStep::Owned(bytes));
                }
                Ok(size)
            }
            Some(ReadStep::Interrupted) => Err(io::ErrorKind::Interrupted.into()),
            None if self.held_open => Err(io::ErrorKind::WouldBlock.into()),
            Some(ReadStep::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Some(ReadStep::Failed) => Err(io::Error::other("the pipe could not be read")),
            Some(ReadStep::Gone) => Err(io::Error::from_raw_os_error(
                rustix::io::Errno::IO.raw_os_error(),
            )),
            Some(ReadStep::Eof) | None => Ok(0),
        }
    }
}

impl Drop for ScriptedPipe {
    fn drop(&mut self) {
        self.record(Event::Closed);
    }
}

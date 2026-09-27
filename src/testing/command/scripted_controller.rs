use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::rc::Rc;

use super::ReadStep;

/// PTYの親側を模す読み書き端。
///
/// 答えを書く前は最初の台本を、書いた後は`after_answer`の台本を読む。どちらも尽きれば
/// `WouldBlock`を返す。実物のmasterと同じく、台本が尽きたことと相手が消えたことを、
/// testが選べる。
#[derive(Default)]
pub struct ScriptedController {
    before: RefCell<VecDeque<ReadStep>>,
    after: RefCell<VecDeque<ReadStep>>,
    answered: Cell<bool>,
    write_answers: Option<RefCell<VecDeque<io::Result<usize>>>>,
    failing_flush: RefCell<Option<io::Error>>,
    written: Rc<RefCell<Vec<u8>>>,
}

impl ScriptedController {
    /// 答える前に見える読み。台本が尽きれば`WouldBlock`を返す。
    pub fn new(steps: impl IntoIterator<Item = ReadStep>) -> ScriptedController {
        ScriptedController {
            before: RefCell::new(steps.into_iter().collect()),
            ..ScriptedController::default()
        }
    }

    /// 答えを書いた後に見える読み。
    pub fn after_answer(self, steps: impl IntoIterator<Item = ReadStep>) -> ScriptedController {
        ScriptedController {
            after: RefCell::new(steps.into_iter().collect()),
            ..self
        }
    }

    /// 書き込みごとに、受け取るbyte数か失敗を順に答える。尽きれば`Ok(0)`を返す。
    pub fn answering_writes(
        self,
        answers: impl IntoIterator<Item = io::Result<usize>>,
    ) -> ScriptedController {
        ScriptedController {
            write_answers: Some(RefCell::new(answers.into_iter().collect())),
            ..self
        }
    }

    /// `flush`をこの失敗で終える。
    pub fn failing_flush(self, error: io::Error) -> ScriptedController {
        ScriptedController {
            failing_flush: RefCell::new(Some(error)),
            ..self
        }
    }

    /// 書けたbyte。
    pub fn written(&self) -> Rc<RefCell<Vec<u8>>> {
        Rc::clone(&self.written)
    }
}

impl Read for ScriptedController {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut queue = if self.answered.get() {
            self.after.borrow_mut()
        } else {
            self.before.borrow_mut()
        };
        match queue.pop_front() {
            Some(ReadStep::Bytes(bytes)) => {
                let size = bytes.len().min(buffer.len());
                buffer[..size].copy_from_slice(&bytes[..size]);
                if size < bytes.len() {
                    queue.push_front(ReadStep::Bytes(&bytes[size..]));
                }
                Ok(size)
            }
            Some(ReadStep::Owned(mut bytes)) => {
                let size = bytes.len().min(buffer.len());
                buffer[..size].copy_from_slice(&bytes[..size]);
                if size < bytes.len() {
                    bytes.drain(..size);
                    queue.push_front(ReadStep::Owned(bytes));
                }
                Ok(size)
            }
            Some(ReadStep::Interrupted) => Err(io::ErrorKind::Interrupted.into()),
            Some(ReadStep::Failed) => Err(io::Error::other("the controller could not be read")),
            Some(ReadStep::Eof) => Ok(0),
            Some(ReadStep::Gone) => Err(io::Error::from_raw_os_error(
                rustix::io::Errno::IO.raw_os_error(),
            )),
            Some(ReadStep::WouldBlock) | None => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
}

impl Write for ScriptedController {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // 答えを書こうとした時点で、以後の読みは`after_answer`の台本へ移る。
        self.answered.set(true);
        let accepted = match &self.write_answers {
            None => bytes.len(),
            Some(answers) => answers
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(0))?
                .min(bytes.len()),
        };
        self.written
            .borrow_mut()
            .extend_from_slice(&bytes[..accepted]);
        Ok(accepted)
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.failing_flush.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

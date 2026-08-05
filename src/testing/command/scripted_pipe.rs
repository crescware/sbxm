use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::testing::outcome::{Checked, Required};

use super::ReadStep;

/// 読みの結果をtestが決めるstream。
///
/// pollが問い合わせる相手は本物のfdのままにする。regular fileは常に読み取り可能と答える
/// ため、「読める状態だと言われたのに読めない」という、pipeでは起こしようのない側の分岐へ
/// 進める。
pub struct ScriptedPipe {
    ready: File,
    steps: VecDeque<ReadStep>,
    interrupt: Option<Arc<AtomicBool>>,
}

impl ScriptedPipe {
    pub fn new(steps: impl IntoIterator<Item = ReadStep>) -> Checked<ScriptedPipe> {
        Ok(ScriptedPipe {
            ready: File::open("/dev/null")
                .required_because("a descriptor that always polls ready")?,
            steps: steps.into_iter().collect(),
            interrupt: None,
        })
    }

    /// 最初の読みと同時にCtrl-Cを届ける。
    pub fn interrupting(mut self, flag: Arc<AtomicBool>) -> ScriptedPipe {
        self.interrupt = Some(flag);
        self
    }
}

impl Read for ScriptedPipe {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if let Some(flag) = self.interrupt.take() {
            flag.store(true, Ordering::SeqCst);
        }
        match self.steps.pop_front() {
            Some(ReadStep::Bytes(bytes)) => {
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            Some(ReadStep::Interrupted) => Err(io::ErrorKind::Interrupted.into()),
            Some(ReadStep::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Some(ReadStep::Failed) => Err(io::Error::other("the pipe could not be read")),
            None => Ok(0),
        }
    }
}

impl AsFd for ScriptedPipe {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.ready.as_fd()
    }
}

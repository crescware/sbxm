use crate::design::ExternalOutput;

/// 外部toolが端末まで届けたものを記録するsink。
#[derive(Debug, Default)]
pub struct RecordedOutput {
    pub relayed: Vec<u8>,
    pub handed_over: usize,
    pub finished: usize,
}

impl RecordedOutput {
    pub fn new() -> RecordedOutput {
        RecordedOutput::default()
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.relayed).into_owned()
    }
}

impl ExternalOutput for RecordedOutput {
    fn relay(&mut self, bytes: &[u8]) {
        self.relayed.extend_from_slice(bytes);
    }

    fn hand_over(&mut self) {
        self.handed_over += 1;
    }

    fn finished(&mut self) {
        self.finished += 1;
    }
}

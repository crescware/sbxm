use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

/// 書き手へ渡したあとも、書かれたbyteを読める書き込み先。
///
/// 書き手が`'static`の書き込み先を求めるとき、testはこれの写しを渡し、手元の写しで
/// 読む。写しはどれも同じbufferを指す。
#[derive(Debug, Clone, Default)]
pub struct SharedBuffer(Rc<RefCell<Vec<u8>>>);

impl SharedBuffer {
    pub fn new() -> SharedBuffer {
        SharedBuffer::default()
    }

    /// 書かれたbyteを、UTF-8として読めるところまで読む。
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.borrow()).into_owned()
    }

    /// 書かれたtextを行ごとに分ける。
    pub fn lines(&self) -> Vec<String> {
        self.text().lines().map(str::to_string).collect()
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

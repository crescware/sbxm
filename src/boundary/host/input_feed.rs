use std::io::{ErrorKind, Result as IoResult, Write};
use std::os::fd::{AsFd, BorrowedFd};

/// 子のstdinへ渡すbyte列と、そのうち書き終えた位置。
///
/// 出力の読み取りと同じく、親threadが待たずに書く。書き手のthreadを残すと、子孫がstdinを
/// 引き継いだまま読まない場合に、その書き手が戻らない。
pub(super) struct InputFeed<'a, W> {
    pipe: Option<W>,
    bytes: &'a [u8],
    written: usize,
}

impl<'a, W: Write> InputFeed<'a, W> {
    pub(super) fn new(pipe: W, bytes: &'a [u8]) -> InputFeed<'a, W> {
        InputFeed {
            pipe: Some(pipe),
            bytes,
            written: 0,
        }
    }

    /// 待たずに書けるだけ書く。
    ///
    /// 書き終えたら書き込み端を閉じ、子へEOFを届ける。子が先に読むのをやめた場合も閉じ、
    /// 結果は子の終了statusに委ねる。
    pub(super) fn feed(&mut self) -> IoResult<()> {
        let Some(pipe) = self.pipe.as_mut() else {
            return Ok(());
        };
        while self.written < self.bytes.len() {
            match pipe.write(&self.bytes[self.written..]) {
                Ok(0) => return Ok(()),
                Ok(written) => self.written += written,
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
                {
                    return Ok(());
                }
                Err(error) if error.kind() == ErrorKind::BrokenPipe => break,
                Err(error) => return Err(error),
            }
        }
        self.pipe = None;
        Ok(())
    }
}

impl<W: Write + AsFd> InputFeed<'_, W> {
    /// まだ書き残しがあれば、書けるようになるのを待つ書き込み端。
    ///
    /// 子が出力を書かないあいだも、子が読んだ分だけすぐに書き足せるよう、読み取り端と
    /// 一緒に待つ。
    pub(super) fn waiting(&self) -> Option<BorrowedFd<'_>> {
        self.pipe.as_ref().map(AsFd::as_fd)
    }
}

use std::io::Write;

use crate::i18n::Catalog;

use crate::design::document::Document;

use crate::design::Block;
use crate::design::policy::StreamPolicy;

use super::{Painter, Trailing};

/// 1 streamへの描画。
pub struct Renderer<'a> {
    writer: Box<dyn Write + 'a>,
    painter: Painter,
    /// 次に何かを書く前に空行が要るか。
    blank_owed: bool,
    /// 直前に書いたのが工程行か。工程は連続させ、あいだに空行を置かない。
    last_was_progress: bool,
    /// 直前に書いたbyteが改行で終わっていないか。
    ///
    /// blockは必ず改行で終わるため、これが立つのは外部toolのbyteを書いた後だけである。
    unterminated: bool,
}

impl<'a> Renderer<'a> {
    pub fn new(writer: impl Write + 'a, policy: StreamPolicy) -> Renderer<'a> {
        Renderer {
            writer: Box::new(writer),
            painter: Painter { policy },
            blank_owed: false,
            last_was_progress: false,
            unterminated: false,
        }
    }

    /// promptのように、rendererを経由せず同じstreamへ書いたことを知らせる。
    ///
    /// 次のblockが空行から始まるようにするためだけに使う。
    pub(super) fn note_external_write(&mut self) {
        self.blank_owed = true;
        self.last_was_progress = false;
    }

    /// 空行を負っているかを答え、その負債を解消する。
    ///
    /// 端末では複数のstreamが同じ場所へ出る。境界の空行を1つに保つため、どのstreamが
    /// 負っていたかをまとめて確かめてから1度だけ書く。
    pub(super) fn take_owed_blank(&mut self) -> bool {
        let owed = self.blank_owed;
        self.blank_owed = false;
        self.last_was_progress = false;
        owed
    }

    /// 空行を1つ書く。
    pub(super) fn write_blank(&mut self) {
        let _ = self.writer.write_all(b"\n");
        let _ = self.writer.flush();
        self.unterminated = false;
    }

    /// 外部toolが出したbyteを、加工せずそのまま書く。
    pub(super) fn write_external(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
        self.unterminated = bytes.last() != Some(&b'\n');
    }

    /// 外部toolの出力が終わったことを知らせる。
    ///
    /// 改行で終わらなかった場合だけ改行を足す。進捗表示は復帰文字で行を上書きするため、
    /// 最後の1行が改行なしで残ることがある。
    pub(super) fn end_external(&mut self) {
        if self.unterminated {
            let _ = self.writer.write_all(b"\n");
            let _ = self.writer.flush();
            self.unterminated = false;
        }
        self.note_external_write();
    }

    /// documentを描き、flushする。
    pub fn write(&mut self, catalog: &Catalog, document: &Document) {
        for block in document.blocks() {
            let progress = matches!(block, Block::Progress(_));
            self.separate(progress);

            let mut buffer = Vec::new();
            let trailing = self.painter.block(catalog, block, &mut buffer);
            let _ = self.writer.write_all(&buffer);

            self.blank_owed = true;
            self.last_was_progress = progress;
            if trailing == Trailing::Blank {
                let _ = self.writer.write_all(b"\n");
                self.blank_owed = false;
            }
        }
        let _ = self.writer.flush();
    }

    /// blockの前に空行を置く。
    fn separate(&mut self, progress: bool) {
        if self.blank_owed && !(progress && self.last_was_progress) {
            let _ = self.writer.write_all(b"\n");
        }
        self.blank_owed = false;
    }
}

#[cfg(test)]
#[path = "renderer_test.rs"]
mod renderer_test;

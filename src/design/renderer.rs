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
}

impl<'a> Renderer<'a> {
    pub fn new(writer: impl Write + 'a, policy: StreamPolicy) -> Renderer<'a> {
        Renderer {
            writer: Box::new(writer),
            painter: Painter { policy },
            blank_owed: false,
            last_was_progress: false,
        }
    }

    pub(super) fn policy(&self) -> StreamPolicy {
        self.painter.policy
    }

    /// promptのように、rendererを経由せず同じstreamへ書いたことを知らせる。
    ///
    /// 次のblockが空行から始まるようにするためだけに使う。
    pub(super) fn note_external_write(&mut self) {
        self.blank_owed = true;
        self.last_was_progress = false;
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

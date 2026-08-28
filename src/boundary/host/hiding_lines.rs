use crate::design::ExternalOutput;

/// 指定した語を含む行を落として中継する。
///
/// 外部toolは自分の使い方を案内する。その案内がsbxm自身の案内と食い違うとき、2つの
/// 案内が並ぶと利用者はどちらに従えばよいかを決められない。行そのものを見せない。
///
/// 判断は行が終わるところまでで決まるため、行が終わるまでのbyteだけを溜める。復帰文字も
/// 行の終わりとして扱うので、同じ行を書き換える進捗表示は溜まらずそのまま流れる。
pub(super) struct HidingLines<'a> {
    inner: &'a mut dyn ExternalOutput,
    phrases: &'static [&'static str],
    /// 行の終わりがまだ来ていないbyte。
    partial: Vec<u8>,
    /// 見える行が続くまで保留した空行。
    ///
    /// 落とした行の跡に空行だけが残ると、sbxmの行との境界が二重になる。
    held: Vec<u8>,
}

impl<'a> HidingLines<'a> {
    pub(super) fn new(
        inner: &'a mut dyn ExternalOutput,
        phrases: &'static [&'static str],
    ) -> HidingLines<'a> {
        HidingLines {
            inner,
            phrases,
            partial: Vec::new(),
            held: Vec::new(),
        }
    }

    fn hides(&self, line: &[u8]) -> bool {
        let text = String::from_utf8_lossy(line);
        self.phrases.iter().any(|phrase| text.contains(phrase))
    }

    fn show(&mut self, line: &[u8]) {
        if self.hides(line) {
            return;
        }
        // 空行だけでは、そのあとに何かが続くのかどうかが決まらない。
        if line.iter().all(u8::is_ascii_whitespace) {
            self.held.extend_from_slice(line);
            return;
        }
        if !self.held.is_empty() {
            let held = std::mem::take(&mut self.held);
            self.inner.relay(&held);
        }
        self.inner.relay(line);
    }
}

impl ExternalOutput for HidingLines<'_> {
    fn relay(&mut self, bytes: &[u8]) {
        self.partial.extend_from_slice(bytes);
        while let Some(end) = self
            .partial
            .iter()
            .position(|byte| *byte == b'\n' || *byte == b'\r')
        {
            let line: Vec<u8> = self.partial.drain(..=end).collect();
            self.show(&line);
        }
    }

    fn hand_over(&mut self) {
        self.inner.hand_over();
    }

    fn finished(&mut self) {
        // 行の終わりが来ないまま出力が終わることがある。同じ判断で最後の1行を扱う。
        let partial = std::mem::take(&mut self.partial);
        if !partial.is_empty() {
            self.show(&partial);
        }
        // 末尾に残った空行は、境界の空行がすぐあとに続くため見せない。
        self.held.clear();
        self.inner.finished();
    }
}

#[cfg(test)]
#[path = "hiding_lines_test.rs"]
mod hiding_lines_test;

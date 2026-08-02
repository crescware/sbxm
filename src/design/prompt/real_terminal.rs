use console::{Key, Term};

use super::{Keys, Screen};

/// 実端末。打鍵の供給元でもあり、描画先でもある。
///
/// promptの実端末依存はここへ集約する。決めていることは「端末でなければ高さを持たない」
/// の一点であり、残りは同名の端末操作をそのまま渡す。
///
/// 端末でない先へ向けても、書いた文字、読めない高さ、押されない打鍵までは確かめられる。
/// 実TTYでの高さ取得・打鍵・cursor移動・消去の効果は、PTY acceptanceが受け持つ。
pub(super) struct RealTerminal {
    term: Term,
}

impl RealTerminal {
    pub(super) fn new(term: Term) -> RealTerminal {
        RealTerminal { term }
    }
}

impl Keys for RealTerminal {
    fn read_key(&mut self) -> std::io::Result<Key> {
        self.term.read_key()
    }
}

impl Screen for RealTerminal {
    fn write_str(&mut self, text: &str) -> std::io::Result<()> {
        self.term.write_str(text)
    }

    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.term.write_line(line)
    }

    fn clear_chars(&mut self, count: usize) -> std::io::Result<()> {
        self.term.clear_chars(count)
    }

    fn clear_last_lines(&mut self, count: usize) -> std::io::Result<()> {
        self.term.clear_last_lines(count)
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        self.term.hide_cursor()
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        self.term.show_cursor()
    }

    /// 端末の高さ。
    ///
    /// 端末でない場合、`Term::size`は既定値を返す。既定値は観測した高さではないため、
    /// 一覧を切る根拠にしない。
    fn rows(&self) -> Option<u16> {
        self.term.is_term().then(|| self.term.size().0)
    }
}

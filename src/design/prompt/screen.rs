/// promptが書く先。
///
/// 一覧を描き直すには、書くことと同じだけ消すことが要る。消す操作をtraitへ載せるのは、
/// 描き直しの手順をpromptが持ち、端末はその操作を提供するだけにするためである。
pub trait Screen {
    /// 改行せずに書く。入力中の一行を伸ばす。
    fn write_str(&mut self, text: &str) -> std::io::Result<()>;

    /// 一行を書いて改行する。
    fn write_line(&mut self, line: &str) -> std::io::Result<()>;

    /// 現在行の末尾から`count`表示列ぶんを消す。
    fn clear_chars(&mut self, count: usize) -> std::io::Result<()>;

    /// 現在行の前に書いた`count`行を消す。
    fn clear_last_lines(&mut self, count: usize) -> std::io::Result<()>;

    fn hide_cursor(&mut self) -> std::io::Result<()>;

    fn show_cursor(&mut self) -> std::io::Result<()>;

    /// 画面の高さ。端末でなければ`None`。
    fn rows(&self) -> Option<u16>;
}

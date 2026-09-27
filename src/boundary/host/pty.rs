use std::io::{self, Read, Write};
use std::process::Stdio;

/// PTYを1つ開き、端末側を整える基本操作。順番と、どこで諦めるかは`run_pty_confirmed`が決める。
pub(crate) trait Pty {
    /// 親が読み書きする側。
    type Controller: Read + Write;
    /// 子へ渡す端末側。
    type Terminal;
    /// 端末側の名前。
    type Name;
    /// 端末側の設定。testは作れない型を持たずに済むよう、関連型にする。
    type Settings;

    fn open_controller(&self) -> io::Result<Self::Controller>;
    /// macOSの`openpt`はCLOEXECを付けないため、開いた後に付ける（契約test C14）。
    fn close_on_exec(&self, controller: &Self::Controller) -> io::Result<()>;
    fn grant(&self, controller: &Self::Controller) -> io::Result<()>;
    /// `unlockpt`。名前はfile lockの`unlock`と重ねない。
    fn unlock_terminal(&self, controller: &Self::Controller) -> io::Result<()>;
    fn terminal_name(&self, controller: &Self::Controller) -> io::Result<Self::Name>;
    /// 制御端末として奪わずに開く。
    fn open_terminal(&self, name: &Self::Name) -> io::Result<Self::Terminal>;
    fn settings(&self, terminal: &Self::Terminal) -> io::Result<Self::Settings>;
    fn make_raw(&self, settings: &mut Self::Settings);
    fn apply_settings(
        &self,
        terminal: &Self::Terminal,
        settings: &Self::Settings,
    ) -> io::Result<()>;
    fn set_size(&self, controller: &Self::Controller, rows: u16, columns: u16) -> io::Result<()>;
    fn controller_nonblocking(&self, controller: &Self::Controller) -> io::Result<()>;
    /// 子の1本のstreamへ渡す、端末側の複製。
    fn terminal_stdio(&self, terminal: &Self::Terminal) -> io::Result<Stdio>;
}

/// `boundary::terminal`が観測した`stream`ごとの`TTY`判定と端末幅。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Terminals {
    pub stdout_is_tty: bool,
    pub stderr_is_tty: bool,
    /// 端末の桁数。読めない場合は`None`。
    pub width: Option<usize>,
}

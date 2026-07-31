/// 公開契約となるexit code。
///
/// CLI parserを含む内部libraryの既定exit codeを公開契約へ透過しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// 成功、または仕様で成功と定めたno-op。
    Success,
    /// 通常error。引数不正、前提不足、設定・状態不正、外部command失敗、安全上の拒否を含む。
    Failure,
    /// Ctrl-CまたはEscによる対話キャンセル。
    Canceled,
}

impl ExitCode {
    /// processのexit status。値域は`0`、`1`、`130`だけである。
    pub fn as_u8(self) -> u8 {
        match self {
            ExitCode::Success => 0,
            ExitCode::Failure => 1,
            ExitCode::Canceled => 130,
        }
    }
}

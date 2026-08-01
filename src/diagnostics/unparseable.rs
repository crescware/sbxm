use crate::design::{Fact, Inline};
use crate::msg;

use super::{Diagnostic, Error, ErrorId};

/// 外部commandの出力を解釈できなかったことを示す。
///
/// どのcommandの出力かと、なぜ読めなかったかを説明文へ連結しない。同じ色で一文に
/// 並ぶと、読み手はまず区切りを探すことになる。両方を事実の行として分ける。
///
/// `detail`は外部が書いた原文か、sbxmが観測した事実の短い記述であり、翻訳しない。
pub fn unparseable(program: &str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalOutputUnparseable,
            msg!("error-external-output-unparseable"),
        )
        .fact(Fact::new(
            msg!("diagnostic-command-label"),
            Inline::important(program),
        ))
        .fact(Fact::new(
            msg!("diagnostic-cause-label"),
            Inline::text(detail),
        )),
    )
}

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

/// current directoryそのものを読めなかったことの診断。
///
/// 読めなかった時点で示せるpathは1つも無い。OSが書いた原因だけを事実として並べる。
pub(super) fn working_directory_unusable(cause: impl std::fmt::Display) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::WorkingDirectoryUnusable,
            msg!("error-working-directory-unusable"),
        )
        .fact(Fact::cause(&cause.to_string())),
    )
}

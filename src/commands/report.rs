use crate::design::Ui;
use crate::diagnostics::{Error, ExitCode};

/// errorを表示し、そのexit codeを返す。
pub fn report(ui: &mut Ui, error: &Error) -> ExitCode {
    ui.error(error);
    error.exit_code()
}

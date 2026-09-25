use crate::design::{Fact, Warning};
use crate::diagnostics::Error;
use crate::msg;

/// 自動保存できなかったことのwarning。
///
/// 保存していない作業がSandboxにだけ残ることを伝え、手で保存し直すcommandを示す。
/// ErrorIdだけでは何が起きたかが伝わらないため、診断の一文も添える。
pub fn save_failed(project: &str, error: &Error) -> Warning {
    let mut warning = Warning::text(msg!("auto-save-failed", project = project));
    for diagnostic in error.diagnostics() {
        warning = warning
            .fact(Fact::cause(diagnostic.id.as_str()))
            .explain(diagnostic.description.clone());
    }
    warning.try_run(format!("sbxm fetch {project}"))
}

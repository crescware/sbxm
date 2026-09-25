use crate::design::{Document, Ui};
use crate::support::bundle::AutoSaved;

/// 自動で保存した結果を示す。保存できなかった場合はwarningとし、操作は止めない。
///
/// stdoutをSSHへ引き渡すcommandもあるため、stderrへ出す。
pub fn auto_saved(ui: &mut Ui, saved: &AutoSaved) {
    match saved {
        AutoSaved::Nothing => {}
        AutoSaved::Saved(message) => ui.stderr(&Document::new().note(message.clone())),
        AutoSaved::Failed(warning) => ui.warning(warning),
    }
}

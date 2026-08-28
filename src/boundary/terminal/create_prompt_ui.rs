use console::Term;

use crate::design::policy::StreamPolicy;
use crate::design::prompt::PromptUi;
use crate::i18n::Locale;

use super::real_terminal::RealTerminal;

/// stderrの実端末をpromptのportへ接続する。
pub(crate) fn create_prompt_ui(locale: Locale, policy: StreamPolicy) -> PromptUi {
    let term = Term::stderr();
    PromptUi::new(
        locale,
        policy,
        Box::new(RealTerminal::new(term.clone())),
        Box::new(RealTerminal::new(term)),
    )
}

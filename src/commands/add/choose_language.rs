use crate::config::{self, GlobalConfig};
use crate::diagnostics::Result;
use crate::i18n::{Locale, shell_locale};
use crate::support::select::ProjectPrompt;

use crate::commands::Context;

use super::ask_language;

/// この実行で使う表示言語を決め、必要なら一度だけ利用者へ訊く。
///
/// 保存済みの`language`があれば訊かない。`--lang`はそのprocessだけのoverrideであり、
/// 永続設定を利用者自身が選ぶという契約と混同しないため、一度限りのpromptを省略しない。
/// stdinとstderrのどちらかがTTYでない実行では訊かず、保存もしない。
pub fn choose_language(
    context: &Context,
    config: &GlobalConfig,
    fallback: Locale,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Locale> {
    if config.language.is_some() || !context.can_prompt {
        return Ok(fallback);
    }
    let chosen = ask_language(prompt, shell_locale().unwrap_or(Locale::SOURCE))?;
    // 言語保存はproject mutationではなく、利用者が明示的に選んだ独立した設定変更である。
    // 以降のvalidationが失敗しても、選んだ言語をrollbackしない。
    config::save_language(context.location, chosen)?;
    Ok(chosen)
}

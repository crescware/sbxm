//! `add`の実行。
//!
//! 最初の対話的な`add`は、永続的な表示言語を選ぶ一度限りの入口でもある。projectや
//! registryへmutationする前に選ばせ、選択結果を保存してから残りの出力へ使う。

use crate::command::RealHost;
use crate::config::{self, GlobalConfig};
use crate::error::{ExitCode, Result};
use crate::i18n::{Catalog, Locale, shell_locale};
use crate::msg;
use crate::paths::ProjectParent;
use crate::support::select::ProjectPrompt;
use crate::ui::Ui;

use super::super::{Context, report};
use super::run::AddRequest;
use super::{Args, print};

pub fn exec(args: &Args, context: &Context, ui: &mut Ui) -> ExitCode {
    let (config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };

    let locale = match choose_language(context, &config, locale, &mut ui.prompt()) {
        Ok(chosen) => chosen,
        Err(error) => return report(ui, &error),
    };
    ui.note_prompt_output();
    ui.set_locale(locale);

    // cwdはsbxmが選ぶ場所ではない。project rootを足す親directoryとして受け取る。
    let parent = match ProjectParent::current() {
        Ok(parent) => parent,
        Err(error) => return report(ui, &error),
    };

    let request = AddRequest {
        repository: args.repository.clone(),
        worktrees: args.worktrees,
        detach: args.detach.clone(),
    };
    match super::run::run(context.location, &parent, &request, &RealHost, ui) {
        Ok(output) => {
            for warning in &output.warnings {
                ui.warning(warning);
            }
            ui.stdout(&print::document(&output));
            ExitCode::Success
        }
        Err(error) => report(ui, &error),
    }
}

/// この実行で使う表示言語を決め、必要なら一度だけ利用者へ訊く。
///
/// 保存済みの`language`があれば訊かない。`--lang`はそのprocessだけのoverrideであり、
/// 永続設定を利用者自身が選ぶという契約と混同しないため、一度限りのpromptを省略しない。
/// stdinとstderrのどちらかがTTYでない実行では訊かず、保存もしない。
pub(super) fn choose_language(
    context: &Context,
    config: &GlobalConfig,
    fallback: Locale,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Locale> {
    if config.language.is_some() || !context.interactivity.can_prompt() {
        return Ok(fallback);
    }
    let chosen = ask_language(prompt, shell_locale().unwrap_or(Locale::SOURCE))?;
    // 言語保存はproject mutationではなく、利用者が明示的に選んだ独立した設定変更である。
    // 以降のvalidationが失敗しても、選んだ言語をrollbackしない。
    config::save_language(context.location, chosen)?;
    Ok(chosen)
}

/// 表示言語を選ばせる。
///
/// 見出しは選択前でも双方の利用者が読める固定の二言語表記とし、選択肢はその言語自身の
/// 自称表記で並べる。system localeから推測した言語を先頭へ置き、初期cursorに載せる。
pub(super) fn ask_language(prompt: &mut dyn ProjectPrompt, guessed: Locale) -> Result<Locale> {
    let mut choices = vec![guessed];
    choices.extend(Locale::ALL.into_iter().filter(|locale| *locale != guessed));

    let items: Vec<String> = choices.iter().map(|locale| locale_name(*locale)).collect();
    let index = prompt.select_one(msg!("prompt-language-heading"), &items)?;
    Ok(*choices
        .get(index)
        .expect("the selection index stays within the offered items"))
}

/// その言語自身のresourceが持つ自称表記。
fn locale_name(locale: Locale) -> String {
    Catalog::new(locale)
        .text("locale-name")
        .unwrap_or_else(|failure| failure.to_string())
}

#[cfg(test)]
#[path = "exec_test.rs"]
mod exec_test;

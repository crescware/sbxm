//! `add`の実行。
//!
//! 最初の対話的な`add`は、永続的な表示言語と案件の名義を選ぶ一度限りの入口でもある。
//! projectやregistryへmutationする前に選ばせ、選択結果を保存してから残りの出力へ使う。

use crate::command::{HostEnvironment, RealHost};
use crate::config::{self, GlobalConfig};
use crate::error::{Diagnostic, Error, ErrorId, ExitCode, Result, fail};
use crate::i18n::{Catalog, Locale, shell_locale};
use crate::metadata::{GitIdentity, validate_git_identity_value};
use crate::msg;
use crate::paths::ProjectParent;
use crate::support::identity;
use crate::support::select::ProjectPrompt;
use crate::ui::{PromptUi, Remediation, Ui};

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

    // 名義は、この実行が何かを作る前に決める。訊くなら選んだ言語で訊く。
    let git_identity =
        match choose_git_identity(context, &config, args, &RealHost, &mut ui.prompt()) {
            Ok(chosen) => chosen,
            Err(error) => return report(ui, &error),
        };
    ui.note_prompt_output();

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
    match super::run::run(
        context.location,
        &parent,
        &request,
        &git_identity,
        &RealHost,
        ui,
    ) {
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

/// 名義を1行ずつ訊くprompt。
///
/// 候補は初期値として置くだけで、確定した値ではない。EscとCtrl-Cはどちらも何も
/// 登録せず終える。
pub trait IdentityPrompt {
    fn git_user_name(&mut self, candidate: &str) -> Result<String>;
    fn git_user_email(&mut self, candidate: &str) -> Result<String>;
}

impl IdentityPrompt for PromptUi {
    fn git_user_name(&mut self, candidate: &str) -> Result<String> {
        self.input(msg!("prompt-git-user-name"), candidate)
    }

    fn git_user_email(&mut self, candidate: &str) -> Result<String> {
        self.input(msg!("prompt-git-user-email"), candidate)
    }
}

/// この案件の名義を決め、必要なら一度だけ利用者へ訊く。
///
/// 宣言があればそれを使い、無ければ保存済みの既定を使う。どちらも無い場合にだけ訊き、
/// 訊けなければ、hostの値で代用せずmutationの前に拒否する。
///
/// `--git-user-name`と`--git-user-email`はそのprocessだけのoverrideであり、`--lang`と
/// 同じく、永続設定を利用者自身が選ぶという契約と混同しないため保存しない。
pub(super) fn choose_git_identity(
    context: &Context,
    config: &GlobalConfig,
    args: &Args,
    host: &dyn HostEnvironment,
    prompt: &mut dyn IdentityPrompt,
) -> Result<GitIdentity> {
    if let Some(declared) = &args.git_identity {
        return Ok(declared.clone());
    }
    if let Some(saved) = &config.git_identity {
        return Ok(saved.clone());
    }
    if !context.interactivity.can_prompt() {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::GitIdentityUndecidable,
                msg!("error-git-identity-undecidable"),
            )
            .remediation(
                Remediation::text(msg!("remediation-git-identity-undecidable")).try_run(
                    "sbxm add <clone-url> --git-user-name <name> --git-user-email <email>",
                ),
            ),
        ));
    }

    let chosen = ask_git_identity(prompt, host)?;
    // 名義の保存はproject mutationではなく、利用者が明示的に選んだ独立した設定変更で
    // ある。以降のvalidationが失敗しても、選んだ名義をrollbackしない。
    config::save_git_identity(context.location, &chosen)?;
    Ok(chosen)
}

/// 名義を選ばせる。
///
/// hostが宣言している値を初期値として置く。読めない場合は空欄で始まり、それ自体は
/// 失敗ではない。
pub(super) fn ask_git_identity(
    prompt: &mut dyn IdentityPrompt,
    host: &dyn HostEnvironment,
) -> Result<GitIdentity> {
    let typed_name = prompt.git_user_name(&identity::candidate_from_host(host, "user.name"))?;
    let typed_email = prompt.git_user_email(&identity::candidate_from_host(host, "user.email"))?;
    Ok(GitIdentity {
        user_name: accept("user.name", &typed_name)?,
        user_email: accept("user.email", &typed_email)?,
    })
}

/// 入力された1行を名義の値として受け取る。
fn accept(field: &str, value: &str) -> Result<String> {
    let value = value.trim();
    match validate_git_identity_value(value) {
        Ok(()) => Ok(value.to_string()),
        Err(detail) => fail(
            ErrorId::InvalidValue,
            msg!("error-git-identity-invalid", field = field, detail = detail),
        ),
    }
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

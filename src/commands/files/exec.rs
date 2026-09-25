//! `files`の実行。

use crate::boundary::host::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::{Error, ExitCode};
use crate::support::select;

use super::{
    super::{Context, apply, report},
    Args, PullOutcome, Pulled, absolute_source, add, adopt, ask_to_adopt, ask_to_apply, host_diff,
    print, pull, remove, visible,
};

pub fn exec(
    args: &Args,
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    let (config, locale) = match context.settings() {
        Ok(pair) => pair,
        Err(error) => return report(ui, &error),
    };
    ui.set_locale(locale);
    prompt.set_locale(locale);
    match args {
        Args::Ls => {
            ui.stdout(&print::list(&config.files));
            ExitCode::Success
        }
        Args::Rm { destination } => match remove(context.location, destination) {
            Ok((path, removed)) => {
                ui.stdout(&print::removed(&path, &removed));
                ExitCode::Success
            }
            Err(error) => report(ui, &error),
        },
        Args::Pull {
            destination,
            project,
        } => {
            if let Err(error) = crate::support::login::require_signed_in(host) {
                return report(ui, &error);
            }
            let mut pulled = match pull(
                context.location,
                &config,
                destination,
                project.as_ref(),
                prompt,
                host,
                context.workspace_root,
            ) {
                Ok(pulled) => pulled,
                Err(error) => return report(ui, &error),
            };
            let code = match decide(&mut pulled, context, ui, host, prompt) {
                Ok(outcome) => {
                    ui.stdout(&print::pull_result(&pulled, &outcome));
                    ExitCode::Success
                }
                Err(error) => report(ui, &error),
            };
            // 取り出した内容は、採用してもしなくても隔離領域に残さない。
            let _ = std::fs::remove_file(&pulled.copy.path);
            code
        }
        Args::Add {
            source,
            destination,
        } => {
            let added = match absolute_source(source)
                .and_then(|source| add(context.location, &config, &source, destination.as_deref()))
            {
                Ok(added) => added,
                Err(error) => return report(ui, &error),
            };
            ui.stdout(&print::added(&added));
            if added.credential_like {
                ui.warning(&print::credential_warning(&added));
            }
            if added.already {
                return ExitCode::Success;
            }
            offer_to_apply(context, ui, host, prompt)
        }
    }
}

/// 取り出したSandbox側の内容を、差分を見せてから採用するかを決める。自動では混ぜない。
fn decide(
    pulled: &mut Pulled,
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> crate::diagnostics::Result<PullOutcome> {
    if pulled.copy.sha256 == pulled.host_sha256 {
        return Ok(PullOutcome::Same);
    }
    let source = pulled.declaration.source.as_path().to_path_buf();
    let diff = host_diff(host, &source, &pulled.copy.path)?;
    ui.stdout(&print::pull_diff(pulled, &visible(&diff)));
    if !context.can_prompt {
        return Ok(PullOutcome::Undecided);
    }
    match ask_to_adopt(prompt, &source, ui.locale()) {
        Ok(true) => Ok(PullOutcome::Adopted(adopt(pulled)?)),
        Ok(false) | Err(Error::Canceled) => Ok(PullOutcome::Kept),
        Err(error) => Err(error),
    }
}

/// 足した宣言を、登録済みの全案件へ今すぐ配置するかを訊き、選ばれれば配置する。
///
/// 宣言は保存済みである。訊けない実行、配置しないと選んだ実行、訊いている途中で
/// やめた実行では、あとで配置する手順を示して成功として終える。
fn offer_to_apply(
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    let count = match select::candidates(context.location) {
        Ok(candidates) => candidates.len(),
        // 宣言は保存済みである。案件を読めなければ訊かず、あとで配置する手順を示す。
        Err(error) => {
            ui.warning(&print::not_offered(&error));
            ui.stdout(&print::apply_hint());
            return ExitCode::Success;
        }
    };
    if count == 0 {
        return ExitCode::Success;
    }
    let chosen = if context.can_prompt {
        match ask_to_apply(prompt, count, ui.locale()) {
            Ok(chosen) => chosen,
            Err(Error::Canceled) => false,
            Err(error) => return report(ui, &error),
        }
    } else {
        false
    };
    if !chosen {
        ui.stdout(&print::apply_hint());
        return ExitCode::Success;
    }
    apply_everywhere(context, ui, host, prompt)
}

/// 足したあとのconfigで、全案件へ宣言fileを配置する。
///
/// `sbxm apply --files --all`と同じ入口を通す。認証の確認、configの読み直し、結果の
/// 示し方を、ここで別に持たない。
fn apply_everywhere(
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    let everywhere = apply::Args {
        project: None,
        all: true,
        files: true,
        force: false,
        worktrees: None,
    };
    apply::exec(&everywhere, context, ui, host, prompt)
}

#[cfg(test)]
#[path = "exec_test.rs"]
mod exec_test;

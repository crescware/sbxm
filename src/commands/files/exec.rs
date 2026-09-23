//! `files`の実行。

use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::{Error, ExitCode};
use crate::support::select;

use super::{
    super::{Context, apply, report},
    Args, absolute_source, add, ask_to_apply, print, remove,
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
        Err(error) => return report(ui, &error),
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
    apply_everywhere(context, ui, host, context.workspace_root)
}

/// 足したあとのconfigで、全案件へ宣言fileを配置する。
fn apply_everywhere(
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> ExitCode {
    if let Err(error) = crate::support::login::require_signed_in(host) {
        return report(ui, &error);
    }
    let config = match context.settings() {
        Ok((config, _)) => config,
        Err(error) => return report(ui, &error),
    };
    match apply::run_all(context.location, &config, false, host, workspace_root, ui) {
        Ok(applied) => apply::print::all_report(ui, &applied),
        Err(error) => report(ui, &error),
    }
}

#[cfg(test)]
#[path = "exec_test.rs"]
mod exec_test;

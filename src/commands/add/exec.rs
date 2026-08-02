use crate::command::RealHost;
use crate::design::Ui;
use crate::diagnostics::ExitCode;
use crate::paths::ProjectParent;

use crate::commands::add::AddRequest;
use crate::commands::add::{Args, print};
use crate::commands::{Context, report};

use super::{choose_git_identity, choose_language};

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
    match crate::commands::add::run::run(
        context.location,
        &parent,
        &request,
        &git_identity,
        &RealHost,
        ui,
    ) {
        Ok(output) => {
            ui.stdout(&print::document(&output));
            ExitCode::Success
        }
        Err(error) => report(ui, &error),
    }
}

#[cfg(test)]
#[path = "exec_test.rs"]
mod exec_test;

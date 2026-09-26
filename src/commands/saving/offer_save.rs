use std::ops::ControlFlow;

use crate::boundary::host::HostEnvironment;
use crate::design::Ui;
use crate::diagnostics::{Error, ErrorId, ExitCode, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::project::ProjectId;
use crate::support::protection;
use crate::support::select::ProjectPrompt;

use super::super::{Context, report};
use super::{save_now, saved_document};

/// 保護の検査が、保存で解けるoriginに無いcommitだけを理由に断ったとき、hostへ保存して
/// 続けるかを訊く。stashやnotesのように保存が運ばないrefが理由に含まれれば訊かない。
///
/// 保存を選べば、Sandboxのcommitをhostの`refs/sbx/<sandbox>/`へ保存し、`Continue`を返す。
/// 呼び出し側はもう一度準備する。hostへ保存したcommitは、保護の検査で失われないものと
/// して数えられる。hostのbranchとtagは動かさない。
/// ほかの理由を含む失敗や、訊けない場面では訊かずに元の失敗を報告する。
///
/// 準備が返した失敗はproject lockをもう手放している。保存はlockを取り直して行う。
pub fn offer_save(
    error: &Error,
    project: &ProjectId,
    context: &Context,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    ui: &mut Ui,
) -> ControlFlow<ExitCode> {
    if !context.can_prompt || !only_unreachable_commits(error) {
        return ControlFlow::Break(report(ui, error));
    }
    // 何を失うのかを見せてから訊く。止めた場合は、この表示が失敗の報告になる。
    ui.error(error);
    let saving = ask_to_save(prompt, ui.locale());
    ui.note_prompt_output();
    match saving {
        Ok(true) => {}
        Ok(false) => return ControlFlow::Break(error.exit_code()),
        Err(failure) => return ControlFlow::Break(report(ui, &failure)),
    }
    match save_now(
        context.location,
        Some(project),
        prompt,
        host,
        context.workspace_root,
    ) {
        Ok(output) => {
            ui.stdout(&saved_document(&output, ui.locale()));
            ControlFlow::Continue(())
        }
        Err(failure) => ControlFlow::Break(report(ui, &failure)),
    }
}

/// hostへ保存すれば解ける失敗か。
fn only_unreachable_commits(error: &Error) -> bool {
    let diagnostics = error.diagnostics();
    !diagnostics.is_empty() && diagnostics.iter().all(protection::saving_resolves)
}

/// hostのrepositoryへ書き足すだけの選択であり、何も消さないため先頭に置く。
fn ask_to_save(prompt: &mut dyn ProjectPrompt, locale: Locale) -> Result<bool> {
    let catalog = Catalog::new(locale);
    let label = |id: &str| match catalog.text(id) {
        Ok(text) => text,
        Err(failure) => failure.to_string(),
    };
    let choices = [
        label("prompt-save-to-host-save"),
        label("prompt-save-to-host-stop"),
    ];
    let index = prompt.select_one(&msg!("prompt-save-to-host-heading"), &choices)?;
    match index {
        0 => Ok(true),
        1 => Ok(false),
        // 候補に対応しない選択はcancelではない。promptの契約違反として区別して報告する。
        _ => Err(Error::new(
            ErrorId::SelectionUnresolved,
            msg!(
                "error-selection-unresolved",
                index = index,
                count = choices.len()
            ),
        )),
    }
}

#[cfg(test)]
#[path = "offer_save_test.rs"]
mod offer_save_test;

//! `init`の実行と出力。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::msg;
use crate::paths;
use crate::ui::{Document, GuidanceItem, Ui};

use super::super::{Context, report};
use super::Mode;
use super::run::{InitRequest, TerminalPrompt};

pub fn exec(mode: &Mode, context: &Context, ui: &mut Ui) -> ExitCode {
    let request = InitRequest {
        mode: mode.clone(),
        lang: context.lang,
        interactivity: context.interactivity,
    };
    let mut prompt = TerminalPrompt::new(ui.prompt());
    let output = match super::run::run(context.location, &request, &RealHost, &mut prompt) {
        Ok(output) => output,
        Err(error) => return report(ui, &error),
    };
    ui.note_prompt_output();

    // 作成したconfigが選んだlocaleで、結果と次の一歩を出す。
    ui.set_locale(output.locale);
    for warning in &output.warnings {
        ui.warning(warning);
    }

    let path = paths::display(&output.config_path);
    let summary = if output.already_initialized {
        msg!("init-already-initialized", path = path)
    } else {
        msg!("init-created", path = path)
    };
    ui.stdout(
        &Document::new()
            .summary(summary)
            .guidance(None, vec![GuidanceItem::Plain(msg!("init-next-step"))])
            .try_command("sbxm status --global"),
    );
    ExitCode::Success
}

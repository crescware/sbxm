use std::ops::ControlFlow;

use crate::boundary::host::HostEnvironment;
use crate::design::Ui;
use crate::diagnostics::{ExitCode, Result};
use crate::project::ProjectId;
use crate::support::select::ProjectPrompt;

use super::{
    super::{Context, report},
    offer_save,
};

/// 破壊操作の準備を行い、保存で解ける理由だけで断られたら、保存を申し出てやり直す。
///
/// 申し出るのは1回だけとする。保存したあとの準備がまだ断るなら、もう訊かずにその
/// 失敗を報告する。`prepare`は呼ぶたびに`project`を選び直さず準備する。
pub fn prepare_offering_save<T>(
    project: &ProjectId,
    context: &Context,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    ui: &mut Ui,
    mut prepare: impl FnMut(&mut dyn ProjectPrompt, &mut Ui) -> Result<T>,
) -> ControlFlow<ExitCode, T> {
    let mut offered = false;
    loop {
        match prepare(prompt, ui) {
            Ok(prepared) => return ControlFlow::Continue(prepared),
            Err(error) if offered => return ControlFlow::Break(report(ui, &error)),
            Err(error) => {
                offered = true;
                if let ControlFlow::Break(code) =
                    offer_save(&error, project, context, host, prompt, ui)
                {
                    return ControlFlow::Break(code);
                }
            }
        }
    }
}

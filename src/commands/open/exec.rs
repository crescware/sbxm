//! `open`の実行と出力。

use crate::boundary::host::HostEnvironment;
use crate::design::{Document, Inline, PromptUi, SilentProgress, Ui, Warning};
use crate::diagnostics::ExitCode;
use crate::msg;
use crate::project::ProjectId;
use crate::support::inventory;

use crate::commands::{present, saving};

use super::super::{Context, report};
use super::Args;

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
    if let Err(error) = crate::support::login::require_signed_in(host) {
        return report(ui, &error);
    }
    let prepared = match super::run::prepare(
        context.location,
        &config,
        args.project.as_ref(),
        args.index,
        host,
        prompt,
        context.workspace_root,
        inventory::Poll::default(),
        ui,
    ) {
        Ok(prepared) => prepared,
        Err(error) => return report(ui, &error),
    };

    // 初回構築を行った実行では、接続先を見せる前に何を作ったかを示す。stdoutはSSHへ
    // 引き渡すため、この報告もstderrへ出す。
    if let Some(output) = &prepared.provisioned {
        for warning in &output.warnings {
            ui.warning(warning);
        }
        ui.stderr(&present::provisioning_output(output, locale));
    }
    for warning in &prepared.warnings {
        ui.warning(warning);
    }

    if let Some(index) = prepared.missing_worktree_index {
        ui.warning(&Warning::text(msg!(
            "warning-open-worktree-not-found",
            index = index
        )));
    }

    // promptで選べた値と接続先が食い違う場合は、接続先を見せる前にその差を述べる。
    if let Some(clamped) = prepared.clamped_worktree_index {
        ui.warning(&Warning::text(msg!(
            "warning-open-worktree-index-clamped",
            requested = clamped.requested,
            index = clamped.opened
        )));
    }

    // 接続先はterminalを引き渡す前に見せる。
    let connecting = Document::new()
        .summary(msg!(
            "open-connecting",
            project = prepared.project,
            sandbox = prepared.sandbox
        ))
        .lines(
            Some(msg!("open-worktrees-heading")),
            prepared
                .worktrees
                .iter()
                .map(Inline::path)
                .map(Into::into)
                .collect(),
        );
    ui.stderr(&present::disk_section(connecting, &prepared.disk));

    let project = ProjectId::parse(&prepared.project);
    // hostにあるrepositoryの案件は、sessionのあいだも保存しておく。端末はSSHが持つため、
    // 進捗も結果も示さない。保存できなかったことは、sessionを閉じたあとの保存が伝える。
    let mut save = || {
        if let Ok(project) = &project {
            let _ = saving::save_first(
                context.location,
                project,
                host,
                context.workspace_root,
                &mut SilentProgress,
            );
        }
    };
    let during: Option<&mut dyn FnMut()> = if prepared.from_host {
        Some(&mut save)
    } else {
        None
    };
    let connected = super::run::connect(host, prepared, ui, during);
    // sessionを閉じたあとにも保存しておく。
    if let Ok(project) = &project {
        let saved = saving::save_first(context.location, project, host, context.workspace_root, ui);
        saving::auto_saved(ui, &saved);
    }
    match connected {
        Ok(()) => ExitCode::Success,
        Err(error) => report(ui, &error),
    }
}

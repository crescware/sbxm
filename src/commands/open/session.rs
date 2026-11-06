use std::time::Duration;

use crate::boundary::host::HostEnvironment;
use crate::commands::{Context, saving};
use crate::design::{Document, SilentProgress, Ui};
use crate::diagnostics::ExitCode;
use crate::msg;
use crate::paths::LOCK_TIMEOUT;

use super::run::Prepared;

/// 接続前と終了後の保存状態を示し、session中に発生した保存失敗も残す。
pub(super) fn session(
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prepared: Prepared,
) -> ExitCode {
    // hostにあるrepositoryの案件は、sessionのあいだも保存しておく。端末はSSHが持つため、
    // 進捗も結果も示さない。保存できなかったことは、sessionを閉じたあとの保存が伝える。
    // lockは待たない。待つあいだはSSHの終了に気付けず、lockを持つ別のcommandも、この
    // 保存を待つことになる。取れなければ次の機会に保存する。
    let saves_to_host = prepared.saves_to_host.clone();
    // 満杯で保存できない状態を、terminalへ渡す前に知らせる。
    if let Some(candidate) = saves_to_host.clone() {
        let saved = saving::save_selected(
            candidate,
            host,
            context.workspace_root,
            LOCK_TIMEOUT,
            context.clock,
            ui,
        );
        saving::auto_saved(ui, &saved);
        if !matches!(saved, crate::support::host_sync::AutoSaved::Failed(_)) {
            ui.stderr(&Document::new().note(msg!(
                "session-backup-current",
                checked = crate::support::host_sync::stamp(context.clock.wall())
            )));
        }
    }
    let pending_warning = std::cell::RefCell::new(None);
    let mut save = saves_to_host.clone().map(|candidate| {
        let pending_warning = &pending_warning;
        move || {
            let saved = saving::save_selected(
                candidate.clone(),
                host,
                context.workspace_root,
                Duration::ZERO,
                context.clock,
                &mut SilentProgress,
            );
            if let crate::support::host_sync::AutoSaved::Failed(warning) = saved {
                *pending_warning.borrow_mut() = Some(warning);
            }
        }
    });
    let during: Option<&mut dyn FnMut()> = match save.as_mut() {
        Some(save) => Some(save),
        None => None,
    };
    let connected = super::run::connect(host, prepared, ui, during);
    if let Err(error) = &connected {
        ui.error(error);
    }
    if let Some(warning) = pending_warning.into_inner() {
        ui.warning(&warning);
    }
    // sessionを閉じたあとにも保存しておく。
    let mut unsaved = false;
    if let Some(candidate) = saves_to_host {
        let saved = saving::save_selected(
            candidate,
            host,
            context.workspace_root,
            LOCK_TIMEOUT,
            context.clock,
            ui,
        );
        if connected.is_err()
            && let crate::support::host_sync::AutoSaved::Failed(warning) = &saved
        {
            let mut secondary = warning.clone();
            secondary.description = msg!("warning-open-ssh-save-secondary");
            secondary.guidance.push(warning.description.clone());
            ui.warning(&secondary);
        } else {
            saving::auto_saved(ui, &saved);
        }
        unsaved = matches!(saved, crate::support::host_sync::AutoSaved::Failed(_));
        if !unsaved {
            ui.stderr(&Document::new().note(msg!(
                "session-backup-current",
                checked = crate::support::host_sync::stamp(context.clock.wall())
            )));
        }
    }
    match connected {
        Ok(()) if unsaved => ExitCode::Failure,
        Ok(()) => ExitCode::Success,
        Err(error) => error.exit_code(),
    }
}

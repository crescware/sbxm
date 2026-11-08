use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::{Document, Ui};
use crate::diagnostics::{Error, Result};
use crate::i18n::Catalog;
use crate::msg;
use crate::support::{inventory, select::ProjectPrompt};

use super::super::Context;
use super::run::{OpenSelection, Prepared, prepare};
use super::ssh_repair::SshRepair;
use super::{Args, connection_error};

/// 対象を1度選び、観測した不足だけを承認付きで直して、接続準備をやり直す。
pub(super) fn prepare_connection(
    args: &Args,
    context: &Context,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    ui: &mut Ui,
) -> Result<Prepared> {
    let selection =
        OpenSelection::resolve(context.location, args.project.as_ref(), args.index, prompt)?;
    let project = crate::project::ProjectId::parse(&selection.candidate.display_id())?;
    let mut attempted = Vec::new();
    let mut restart_required = false;
    loop {
        let error = if restart_required {
            SshRepair::restart_required()
        } else {
            match prepare(
                context.location,
                config,
                Some(&project),
                selection.index,
                host,
                prompt,
                context.workspace_root,
                inventory::Poll::standard(context.clock),
                ui,
            ) {
                Ok(prepared) => return Ok(prepared),
                Err(error) => error,
            }
        };
        let Some(repair) = SshRepair::for_error(&error)
            .filter(|repair| context.can_prompt && !attempted.contains(repair))
        else {
            return Err(connection_error(error, &selection));
        };
        repair
            .verify_policy(host)
            .map_err(|failure| connection_error(failure, &selection))?;
        let mut plan = Document::new();
        for diagnostic in error.diagnostics() {
            plan = plan.note(diagnostic.description.clone());
        }
        ui.stderr(
            &plan
                .note(repair.description())
                .try_command(repair.command()),
        );
        let catalog = Catalog::new(ui.locale());
        let label = |key| catalog.text(key).unwrap_or_else(|error| error.to_string());
        let choices = [
            label("prompt-ssh-repair-stop"),
            label("prompt-ssh-repair-run"),
        ];
        let answer = prompt.select_one(
            &msg!("prompt-ssh-repair-heading"),
            &choices,
            crate::design::Filter::None,
        );
        ui.note_prompt_output();
        match answer {
            Err(Error::Canceled) | Ok(0) => return Err(connection_error(error, &selection)),
            Err(failure) => return Err(connection_error(failure, &selection)),
            Ok(1) => {}
            Ok(other) => return Err(crate::support::select::unresolved(other, choices.len())),
        }
        // prepareがErrを返したので、project lockもleaseも既に解放されている。
        let target = selection.candidate.reload()?.sandbox_name().to_string();
        restart_required = repair
            .execute(host, context.clock, &target, restart_required)
            .map_err(|failure| connection_error(failure, &selection))?;
        attempted.push(repair);
    }
}

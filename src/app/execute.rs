use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::boundary::host::{HostEnvironment, RealHost, Traced};
use crate::boundary::os::SystemClock;
use crate::boundary::terminal::{create_prompt_ui, create_trace, create_ui};
use crate::commands::{Command, Context};
use crate::design::Document;
use crate::diagnostics::{ExitCode, Result};
use crate::support::sandbox::WORKSPACE_ROOT;

use super::invocation::Invocation;

/// parse結果を利用者へ提示し、commandをapplication境界から実行する。
///
/// この1回の起動が持つ材料はここで尽きる。`Invocation`は値で受け取り、実行の間だけ
/// 保持する。helpとversionもほかのcommandと同じ1つのmatchで受け、提示先だけが違う。
///
/// `--verbose`では、工程と外部commandの経過を並べる`Trace`を1つ作り、`Ui`、
/// `PromptUi`、hostへ持たせる。hostは`RealHost`を包むdecoratorになり、実行の判断は
/// 変わらない。
#[allow(clippy::needless_pass_by_value)]
pub(super) fn execute(invocation: Invocation, command: Result<Command>) -> ExitCode {
    let locale = invocation.locale();
    let policy = invocation.rendering_policy();
    let mut ui = create_ui(locale, policy);
    let trace = invocation.verbose().then(|| {
        Rc::new(RefCell::new(create_trace(
            locale,
            policy.stderr,
            Rc::new(SystemClock),
        )))
    });
    if let Some(trace) = &trace {
        ui.attach_trace(Rc::clone(trace));
    }

    let command = match command {
        Ok(command) => command,
        Err(error) => {
            ui.error(&error);
            return error.exit_code();
        }
    };

    let context = Context {
        location: invocation.location(),
        workspace_root: Path::new(WORKSPACE_ROOT),
        clock: &SystemClock,
        locale,
        can_prompt: invocation.can_prompt(),
    };
    let real = &RealHost;
    let traced = trace
        .as_ref()
        .map(|trace| Traced::new(real, Rc::clone(trace)));
    let host: &dyn HostEnvironment = match &traced {
        Some(traced) => traced,
        None => real,
    };
    let mut prompt = create_prompt_ui(locale, policy.stderr);
    if let Some(trace) = &trace {
        prompt.attach_trace(Rc::clone(trace));
    }

    let code = match command {
        Command::Help(text) | Command::Version(text) => {
            ui.stdout(&Document::new().verbatim(text));
            ExitCode::Success
        }
        Command::Add(args) => {
            crate::commands::add::exec(&args, &context, &mut ui, host, &mut prompt)
        }
        Command::Apply(args) => {
            crate::commands::apply::exec(&args, &context, &mut ui, host, &mut prompt)
        }
        Command::Files(args) => {
            crate::commands::files::exec(&args, &context, &mut ui, host, &mut prompt)
        }
        Command::Sync(project) => {
            crate::commands::sync::exec(project.as_ref(), &context, &mut ui, host, &mut prompt)
        }
        Command::Guide(args) => {
            crate::commands::guide::exec(&args, &context, &mut ui, host, &mut prompt)
        }
        Command::Repair(project) => {
            crate::commands::repair::exec(project.as_ref(), &context, &mut ui, host, &mut prompt)
        }
        Command::Rebuild(project) => {
            crate::commands::rebuild::exec(project.as_ref(), &context, &mut ui, host, &mut prompt)
        }
        Command::Open(args) => {
            crate::commands::open::exec(&args, &context, &mut ui, host, &mut prompt)
        }
        Command::Stop(projects) => {
            crate::commands::stop::exec(&projects, &context, &mut ui, host, &mut prompt)
        }
        Command::Ls => crate::commands::ls::exec(&context, &mut ui, host),
        Command::Status(scope) => {
            crate::commands::status::exec(&scope, &context, &mut ui, host, &mut prompt)
        }
        Command::Destroy(args) => {
            crate::commands::destroy::exec(&args, &context, &mut ui, host, &mut prompt)
        }
    };
    if let Some(trace) = &trace {
        trace.borrow_mut().finish();
    }
    code
}

#[cfg(test)]
#[path = "execute_test.rs"]
mod execute_test;

use crate::design::Ui;
use crate::diagnostics::ExitCode;

use super::{Command, Context};

/// commandを実行し、結果を表示してexit codeを返す。
///
/// `Ui`はworkflow objectへ広げず、明示的な引数として渡す。
pub fn dispatch(command: &Command, context: &Context, ui: &mut Ui) -> ExitCode {
    match command {
        Command::Add(args) => crate::commands::add::exec(args, context, ui),
        Command::Apply(args) => crate::commands::apply::exec(args, context, ui),
        Command::Prepare(project) => crate::commands::prepare::exec(project, context, ui),
        Command::Rebuild(project) => crate::commands::rebuild::exec(project, context, ui),
        Command::Open(project) => crate::commands::open::exec(project.as_ref(), context, ui),
        Command::Stop(projects) => crate::commands::stop::exec(projects, context, ui),
        Command::Ls => crate::commands::ls::exec(context, ui),
        Command::Status(scope) => crate::commands::status::exec(scope, context, ui),
        Command::Destroy(args) => crate::commands::destroy::exec(args, context, ui),
    }
}

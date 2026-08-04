use crate::command::HostEnvironment;
use crate::design::{PromptUi, Ui};
use crate::diagnostics::ExitCode;

use super::{Command, Context};

/// commandを実行し、結果を表示してexit codeを返す。
///
/// `Ui`はworkflow objectへ広げず、明示的な引数として渡す。hostとpromptも同じとし、
/// 実体を選ぶのは呼び手に残す。訊かないcommandへはpromptを渡さない。
pub fn dispatch(
    command: &Command,
    context: &Context,
    ui: &mut Ui,
    host: &dyn HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    match command {
        Command::Add(args) => crate::commands::add::exec(args, context, ui, host, prompt),
        Command::Apply(args) => crate::commands::apply::exec(args, context, ui, host, prompt),
        Command::Prepare(project) => {
            crate::commands::prepare::exec(project.as_ref(), context, ui, host, prompt)
        }
        Command::Rebuild(project) => {
            crate::commands::rebuild::exec(project.as_ref(), context, ui, host, prompt)
        }
        Command::Open(project) => {
            crate::commands::open::exec(project.as_ref(), context, ui, host, prompt)
        }
        Command::Stop(projects) => crate::commands::stop::exec(projects, context, ui, host, prompt),
        Command::Ls => crate::commands::ls::exec(context, ui, host),
        Command::Status(scope) => crate::commands::status::exec(scope, context, ui, host, prompt),
        Command::Destroy(args) => crate::commands::destroy::exec(args, context, ui, host, prompt),
    }
}

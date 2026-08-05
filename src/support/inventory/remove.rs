use std::time::Instant;

use crate::command::{EnvPolicy, HostEnvironment, TerminalCommand, TimeoutClass};
use crate::diagnostics::Result;
use crate::msg;
use crate::project::SandboxName;

use crate::design::ProgressSink;
use crate::support::daemon;

use super::{Poll, single, still_present};

/// Sandboxを削除し、一覧から消えるまで待つ。
///
/// commandの戻り値だけを不在の根拠にしない。`force`はデータ保護検査を省略した
/// 削除であり、runtimeへ渡す引数だけが変わる。
pub fn remove(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    // `--force`が省くのは`sbx`の確認promptだけである。削除してよいかはsbxmが先に
    // 判定しており、`destroy`は自前の確認も済ませている。非対話で走る実行では
    // promptに答える手段がなく、対話実行でも二度訊くことになる。
    progress.step(msg!("progress-removing-sandbox"));
    let args = ["rm", "--force", name.as_str()];
    let command = TerminalCommand::relayed("sbx", &args)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run_with_terminal(&command, progress)?
        .require_success()?;

    let deadline = Instant::now() + poll.limit;
    loop {
        if single(&daemon::list(host)?, name.as_str())?.is_none() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(still_present(name));
        }
        std::thread::sleep(poll.interval);
    }
}

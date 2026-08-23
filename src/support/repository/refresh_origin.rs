use crate::boundary::host::{CommandOutcome, HostEnvironment};
use crate::design::ProgressSink;
use crate::diagnostics::Result;

use crate::support::sandbox;

use super::TagFollowing;

/// bare repositoryのoriginを`fetch --prune`で最新化する。
///
/// 進捗を見せる呼び出しは`progress`へ`Some`を渡して`--progress`付きで中継し、
/// 見せない呼び出しは`None`を渡して静かに実行する。`tags`が`Disabled`なら`--no-tags`を
/// 足し、opportunisticなtag追従によるローカルref書き込みを止める。fetchの引数は
/// この関数だけが持ち、呼び出し側ごとに別実装を増やさない。
pub fn refresh_origin(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    tags: TagFollowing,
    progress: Option<&mut dyn ProgressSink>,
) -> Result<CommandOutcome> {
    let mut args = vec!["git", "--git-dir", git_dir, "fetch", "--prune"];
    if tags == TagFollowing::Disabled {
        args.push("--no-tags");
    }
    if let Some(progress) = progress {
        args.push("--progress");
        args.push("origin");
        sandbox::exec_with_progress(host, sandbox, &args, progress)
    } else {
        args.push("origin");
        sandbox::exec(host, sandbox, &args)
    }
}

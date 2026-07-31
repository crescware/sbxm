use crate::msg;
use crate::paths::{self, ProjectPaths};
use crate::project::SandboxName;

use crate::support::inventory::ProjectState;
use crate::support::secret;

use super::Target;

/// 削除対象。
///
/// pathと同じく、そこに何かがある場合に消すものとして並べる。存在の有無で行を出し
/// 分けると、確認の前に見せる内容がhostへの問い合わせの成否に左右される。
pub(super) fn removes(
    paths: &ProjectPaths,
    name: &SandboxName,
    state: ProjectState,
) -> Vec<Target> {
    let mut removes = Vec::new();
    if state != ProjectState::NotCreated {
        removes.push(Target::Described(msg!(
            "destroy-target-sandbox",
            sandbox = name
        )));
    }
    removes.push(Target::Described(msg!(
        "destroy-target-secret",
        sandbox = name,
        env = secret::GITHUB_TOKEN_ENV
    )));
    removes.push(Target::Path(paths::display(&paths.metadata_file())));
    removes.push(Target::Path(paths::display(&paths.lock_file())));
    removes.push(Target::Path(paths::display(&paths.cache_dir())));
    removes
}

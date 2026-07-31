use crate::testing::outcome::Checked;

use crate::metadata::CreationMode;
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::COMMIT;

use super::layout;

/// worktreeの検査を通る応答。
pub fn worktree_host(mode: CreationMode, count: u32) -> Checked<InnerCommandSandbox> {
    let git_dir = layout()?.bare_git_dir();
    let mut host = InnerCommandSandbox::new().answering(
        &format!("git --git-dir {git_dir} rev-parse refs/remotes/origin/develop"),
        &format!("{COMMIT}\n"),
    );
    for index in 0..count {
        let path = layout()?.worktree(index);
        host = host.answering(
            &format!("git -C {path} rev-parse HEAD"),
            &format!("{COMMIT}\n"),
        );
        host = host.answering(
            &format!("git -C {path} rev-parse --path-format=absolute --git-common-dir"),
            &format!("{}\n", layout()?.bare_git_dir()),
        );
        host = match mode {
            CreationMode::Attached => host.answering(
                &format!("git -C {path} symbolic-ref -q HEAD"),
                "refs/heads/develop\n",
            ),
            CreationMode::Detached => host.failing(&format!("git -C {path} symbolic-ref -q HEAD")),
        };
    }
    Ok(host)
}

use crate::testing::outcome::Checked;

use crate::support::repository::FETCH_REFSPEC;
use crate::testing::sandbox::InnerCommandSandbox;

use super::layout;

/// bare cloneの検査を通る応答。
pub fn healthy_clone() -> Checked<InnerCommandSandbox> {
    let git_dir = layout()?.bare_git_dir();
    Ok(InnerCommandSandbox::new()
        .answering(
            &format!("git --git-dir {git_dir} rev-parse --is-bare-repository"),
            "true\n",
        )
        .answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.fetch"),
            &format!("{FETCH_REFSPEC}\n"),
        ))
}

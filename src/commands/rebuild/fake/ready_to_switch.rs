use crate::testing::host::FakeSbx;
use crate::testing::value::COMMIT;

/// git、GitHub secret、Sandbox内部の検証まで一通り応答するhostを組み立てる。
pub fn ready_to_switch(host: FakeSbx, name: &str, git_dir: &str, worktree: &str) -> FakeSbx {
    super::verified(host, name)
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} rev-parse --is-bare-repository"),
            0,
            "true\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.url"),
            0,
            "https://github.com/example-org/example-repo.git\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.fetch"),
            0,
            "+refs/heads/*:refs/remotes/origin/*\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} rev-parse refs/remotes/origin/main"),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {worktree} rev-parse HEAD"),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git -C {worktree} rev-parse --path-format=absolute --git-common-dir"
            ),
            0,
            &format!("{git_dir}\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {worktree} symbolic-ref -q HEAD"),
            0,
            "refs/heads/main\n",
        )
}

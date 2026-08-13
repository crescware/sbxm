use crate::project::SandboxLayout;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::Checked;
use crate::testing::project::{Fixture, Registered};
use crate::testing::value::COMMIT;

/// 検査を通るworktreeを持つhost。
pub fn clean_host(fixture: &Fixture, project: &Registered) -> Checked<FakeSbx> {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let bare_git_dir = layout.bare_git_dir();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    Ok(answering_origin_observation(
        FakeSbx::listing(&format!(r#"{{"sandboxes":[{}]}}"#, fixture.entry(project, "running")?))
        .answering("version --format {{.Server.Version}}", 0, "27.0.3\n")
        .answering(
            &format!(
                "exec {name} -- git --git-dir {bare_git_dir} worktree list --porcelain -z",
            ),
            0,
            &format!(
                "worktree {}\0bare\0\0worktree {managed}\0branch refs/heads/main\0\0",
                layout.bare_root()
            ),
        )
        .answering(
            &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
            0,
            "",
        )
        // 無視対象pathも、ローカル所有refも、追加remoteも、reflogだけのcommitも無い。
        // 層Bのcollectorが読むcommandは、素通りに見えないよう空の応答を明示する。
        .answering(
            &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --ignored=traditional"),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {} for-each-ref --format=%(refname)%09%(objectname)%09%(upstream) refs/heads/ refs/tags/ refs/notes/ refs/stash",
                layout.bare_git_dir()
            ),
            0,
            "",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {} remote", layout.bare_git_dir()),
            0,
            "origin\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {} rev-list --walk-reflogs --all",
                layout.bare_git_dir()
            ),
            0,
            "",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {} rev-list --all", layout.bare_git_dir()),
            0,
            "",
        )
        .answering(
            &format!("exec {name} -- git -C {managed} rev-parse --git-dir"),
            0,
            &format!("{managed}/.git\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {managed} rev-parse HEAD"),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
            0,
            "main\n",
        )
        .answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --symbolic-full-name @{{upstream}}"
            ),
            0,
            "refs/remotes/origin/main\n",
        )
        // 進行中のGit操作を示すfileはない。
        .answering(&format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/CHERRY_PICK_HEAD"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/REVERT_HEAD"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/BISECT_LOG"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/rebase-merge"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/rebase-apply"), 1, ""),
        name,
        &bare_git_dir,
    ))
}

/// 権威あるorigin観測が、HEADのtipをそのまま持つoriginを見る応答。
///
/// checkout中のbranchはそのupstreamから到達できるため、`Reachability::Pushed`になる。
fn answering_origin_observation(host: FakeSbx, name: &str, bare_git_dir: &str) -> FakeSbx {
    host.answering(
        &format!("exec {name} -- git --git-dir {bare_git_dir} config --get remote.origin.url"),
        0,
        "https://github.com/example-org/example-repo.git\n",
    )
    .answering(
        &format!("exec {name} -- git --git-dir {bare_git_dir} fetch --prune origin"),
        0,
        "",
    )
    .answering(
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
        ),
        0,
        &format!("refs/remotes/origin/main\t{COMMIT}\n"),
    )
    .answering(
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
        ),
        0,
        "refs/remotes/origin/main\n",
    )
}

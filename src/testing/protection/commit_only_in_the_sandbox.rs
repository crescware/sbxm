use crate::project::SandboxLayout;
use crate::testing::host::FakeSbx;
use crate::testing::project::Registered;
use crate::testing::value::COMMIT;

/// `clean_host`のworktreeのHEADを、originに無くhostへ保存すれば検査を通すcommitにする。
///
/// 最初の検査では、HEADはoriginのどのrefからも届かない。hostの名前空間に保存した
/// commitが現れたあとは、Sandboxへ一時refとして置いたそのcommitから届く。
pub fn commit_only_in_the_sandbox(host: FakeSbx, project: &Registered) -> FakeSbx {
    let name = project.sandbox.as_str();
    let git_dir = SandboxLayout::new(project.metadata.canonical_id()).bare_git_dir();
    host.answering(
        &format!(
            "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
        ),
        0,
        "",
    )
    .answering(
        &format!(
            "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/ refs/sbxm/saved/"
        ),
        0,
        "refs/sbxm/saved/0\n",
    )
    .answering_in_turn(
        &format!("for-each-ref --format=%(refname) %(objectname) refs/sbx/{name}/"),
        &[(0, ""), (0, &format!("refs/sbx/{name}/heads/main {COMMIT}\n"))],
    )
}

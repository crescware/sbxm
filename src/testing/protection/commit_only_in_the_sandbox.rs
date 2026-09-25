use crate::project::SandboxLayout;
use crate::testing::host::FakeSbx;
use crate::testing::project::Registered;
use crate::testing::value::COMMIT;

/// `clean_host`のworktreeのHEADを、originに無くhostへ保存すれば検査を通すcommitにする。
///
/// HEADはoriginのどのrefからも届かない。最初の検査では、hostの名前空間にも保存済みの
/// 先端が無い。保存したあとは、hostのrepositoryでその先端から届く。
pub fn commit_only_in_the_sandbox(host: FakeSbx, project: &Registered) -> FakeSbx {
    let name = project.sandbox.as_str();
    let git_dir = SandboxLayout::new(project.metadata.canonical_id()).bare_git_dir();
    let saved = format!("refs/sbx/{name}/heads/main");
    host.answering(
        &format!(
            "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
        ),
        0,
        "",
    )
    .answering_in_turn(
        &format!("for-each-ref --format=%(refname) %(objectname) refs/sbx/{name}/"),
        &[(0, ""), (0, &format!("{saved} {COMMIT}\n"))],
    )
    .answering(
        &format!("for-each-ref --format=%(refname) --contains={COMMIT} refs/sbx/{name}/"),
        0,
        &format!("{saved}\n"),
    )
}

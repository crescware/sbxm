use std::collections::BTreeSet;
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::project::SandboxName;
use crate::support::host_sync;
use crate::support::repository::host_git;

use super::{OriginObservation, host_label};

/// originの観測へ、hostのrepositoryの`refs/sbx/<sandbox>/`に保存済みの先端を足す。
///
/// 保存済みのcommitは、originへ届いていなくてもSandboxを消して失われない。先端は
/// `host:<ref>`と表し、候補のcommitへ届くかはhostのrepositoryで求める。Sandboxの中には
/// 何も置かない。読めない先端やhostに無いcommitからは届くと言えず、保護は拒否する
/// 側へ倒れる。観測できなかったoriginには足さない。
pub(super) fn add_saved(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &SandboxName,
    observation: OriginObservation,
) -> OriginObservation {
    let OriginObservation::Observed {
        mut tips,
        mut reachable_from,
    } = observation
    else {
        return observation;
    };
    let saved = host_sync::saved_tips(host, repository, sandbox).unwrap_or_default();
    if !saved.is_empty() {
        let namespace = host_sync::saved_namespace(sandbox.as_str());
        for tip in saved {
            tips.insert(host_label(&tip.reference), tip.commit);
        }
        for (commit, origins) in &mut reachable_from {
            origins.extend(saved_reaching(host, repository, &namespace, commit));
        }
    }
    OriginObservation::Observed {
        tips,
        reachable_from,
    }
}

/// `commit`へ届く保存済みの先端の表記。hostに無いcommitや読めないrepositoryからは
/// 何も届かない。
fn saved_reaching(
    host: &dyn HostEnvironment,
    repository: &Path,
    namespace: &str,
    commit: &str,
) -> BTreeSet<String> {
    let contains = format!("--contains={commit}");
    let listed = host_git(
        host,
        repository,
        &["for-each-ref", "--format=%(refname)", &contains, namespace],
        None,
        TimeoutClass::LocalFilesystem,
    );
    match listed {
        Ok(outcome) if outcome.success() => outcome
            .stdout_text()
            .lines()
            .filter(|line| !line.is_empty())
            .map(host_label)
            .collect(),
        _ => BTreeSet::new(),
    }
}

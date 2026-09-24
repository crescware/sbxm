use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::project::SandboxName;

use crate::support::repository::host_git;

use super::{CommitCandidate, OriginObservation, UnobservableReason};

/// hostのbranchを示す表記。Sandboxのupstreamと同じ`refs/remotes/origin/`へ揃える。
const ORIGIN_REFS_NAMESPACE: &str = "refs/remotes/origin/";

/// sbxmがhostへ保存した先端を示す表記の接頭辞。
const HOST_LABEL_PREFIX: &str = "host:";

/// hostにあるrepositoryを登録した案件で、hostのrepositoryをoriginとして観測する。
///
/// Sandboxのoriginはhostから送ったbundleであり、送ったあとにhostで消したbranchも
/// 持ち続ける。bundleから辿れても、Sandboxを消したあとに残るとは限らない。hostの
/// branch、tag、sbxmが`refs/sbx/<sandbox>/`へ保存した先端から辿れるcommitだけを、
/// 失われないものとする。hostに無いcommitは、どこからも辿れないものとする。
///
/// hostのbranchは`refs/remotes/origin/<branch>`、tagは`refs/tags/<tag>`、保存した先端は
/// `host:<ref>`と表す。Sandboxの中は読まず、hostのrepositoryも書き換えない。
pub fn observe_host_origin(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &SandboxName,
    candidates: &[CommitCandidate],
) -> Result<OriginObservation> {
    let saved = format!("refs/sbx/{}/", sandbox.as_str());
    let scopes = ["refs/heads/", "refs/tags/", saved.as_str()];

    let mut listing = vec!["for-each-ref", "--format=%(refname) %(objectname)"];
    listing.extend(scopes);
    let listed = git(host, repository, &listing)?;
    if !listed.success() {
        return Ok(unreadable());
    }
    let tips: BTreeMap<String, String> = listed
        .stdout_text()
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(reference, commit)| (label(reference), commit.to_string()))
        .collect();

    let mut reachable_from = BTreeMap::new();
    let unique: BTreeSet<&str> = candidates.iter().map(CommitCandidate::commit).collect();
    for commit in unique {
        let Some(origins) = reaching(host, repository, commit, &scopes)? else {
            return Ok(unreadable());
        };
        reachable_from.insert(commit.to_string(), origins);
    }
    Ok(OriginObservation::Observed {
        tips,
        reachable_from,
    })
}

/// `commit`へ到達できるhostのrefの表記。hostに無いcommitは空とする。hostのrepositoryを
/// 読めなければ`None`を返す。
fn reaching(
    host: &dyn HostEnvironment,
    repository: &Path,
    commit: &str,
    scopes: &[&str],
) -> Result<Option<BTreeSet<String>>> {
    let object = format!("{commit}^{{commit}}");
    if !git(host, repository, &["cat-file", "-e", &object])?.success() {
        return Ok(Some(BTreeSet::new()));
    }
    let contains = format!("--contains={commit}");
    let mut args = vec!["for-each-ref", "--format=%(refname)", contains.as_str()];
    args.extend_from_slice(scopes);
    let outcome = git(host, repository, &args)?;
    if !outcome.success() {
        return Ok(None);
    }
    Ok(Some(
        outcome
            .stdout_text()
            .lines()
            .filter(|line| !line.is_empty())
            .map(label)
            .collect(),
    ))
}

fn git(
    host: &dyn HostEnvironment,
    repository: &Path,
    args: &[&str],
) -> Result<crate::boundary::host::CommandOutcome> {
    host_git(host, repository, args, None, TimeoutClass::LocalFilesystem)
}

/// hostのref名を、Sandboxのupstreamと突き合わせられる表記へ写す。
fn label(reference: &str) -> String {
    if let Some(branch) = reference.strip_prefix("refs/heads/") {
        format!("{ORIGIN_REFS_NAMESPACE}{branch}")
    } else if reference.starts_with("refs/tags/") {
        reference.to_string()
    } else {
        format!("{HOST_LABEL_PREFIX}{reference}")
    }
}

fn unreadable() -> OriginObservation {
    OriginObservation::Unobservable {
        reason: UnobservableReason::HostRepositoryUnreadable,
    }
}

#[cfg(test)]
#[path = "observe_host_origin_test.rs"]
mod observe_host_origin_test;

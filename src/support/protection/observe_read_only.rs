use std::collections::{BTreeMap, BTreeSet};

use crate::boundary::host::{CommandOutcome, HostEnvironment};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::sandbox;

use super::{CommitCandidate, OriginObservation, UnobservableReason};

const ORIGIN_REFS_NAMESPACE: &str = "refs/remotes/origin/";

/// 既にSandbox内にあるorigin refとobjectだけを使って、読み取り専用に観測する。
///
/// statusから呼ばれるため、originへfetchしない。local remote-tracking refが無い、
/// そのtip objectが無い、candidateのobject graphを検査できない、といった場合は
/// Unreachableへ丸めず、ReadOnlyDataInsufficientを返す。
pub fn observe_read_only(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    layout: &SandboxLayout,
    candidates: &[CommitCandidate],
) -> Result<OriginObservation> {
    let scope = ObservationScope {
        sandbox: sandbox.as_str(),
        git_dir: layout.bare_git_dir(),
    };

    if !origin_configured(host, &scope)? {
        return Ok(unobservable(UnobservableReason::OriginMissing));
    }

    let tips = match origin_tips(host, &scope)? {
        Ok(tips) if !tips.is_empty() => tips,
        // 空のtipsは、fetchしていないためにローカルへ何も無いだけかもしれず、originに
        // 本当にbranchが無いとは断定できない。読み取り専用観測ではReadOnlyDataInsufficient
        // へ丸める。一方、advertisementの解釈自体に失敗した場合はデータ不足とは別の
        // 原因であり、その理由をそのまま伝える。
        Ok(_) => return Ok(unobservable(UnobservableReason::ReadOnlyDataInsufficient)),
        Err(reason) => return Ok(unobservable(reason)),
    };

    // for-each-ref can list a remote-tracking ref whose object is no longer present in the
    // local object database. Without this check, a failed ancestry query could be mistaken for
    // a known-unreachable commit.
    for tip in tips.values() {
        if !object_present(host, &scope, tip)? {
            return Ok(unobservable(UnobservableReason::ReadOnlyDataInsufficient));
        }
    }

    let mut reachable_from: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let unique_commits: BTreeSet<&str> = candidates.iter().map(CommitCandidate::commit).collect();
    for commit in unique_commits {
        if !object_present(host, &scope, commit)? {
            return Ok(unobservable(UnobservableReason::ReadOnlyDataInsufficient));
        }
        match reaching_origin_refs(host, &scope, commit)? {
            Ok(origins) => {
                reachable_from.insert(commit.to_string(), origins);
            }
            Err(reason) => return Ok(unobservable(reason)),
        }
    }

    Ok(OriginObservation::Observed {
        tips,
        reachable_from,
    })
}

struct ObservationScope<'a> {
    sandbox: &'a str,
    git_dir: String,
}

fn unobservable(reason: UnobservableReason) -> OriginObservation {
    OriginObservation::Unobservable { reason }
}

fn origin_configured(host: &dyn HostEnvironment, scope: &ObservationScope<'_>) -> Result<bool> {
    let outcome = sandbox::exec(
        host,
        scope.sandbox,
        &[
            "git",
            "--git-dir",
            &scope.git_dir,
            "config",
            "--get",
            "remote.origin.url",
        ],
    )
    .map_err(|error| reclassify(&error, scope))?;
    match sandbox::inner_exit_code(&outcome) {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(unobservable_command(&outcome, scope)),
    }
}

fn origin_tips(
    host: &dyn HostEnvironment,
    scope: &ObservationScope<'_>,
) -> Result<std::result::Result<BTreeMap<String, String>, UnobservableReason>> {
    let outcome = sandbox::exec(
        host,
        scope.sandbox,
        &[
            "git",
            "--git-dir",
            &scope.git_dir,
            "for-each-ref",
            "--format=%(refname)%09%(objectname)",
            ORIGIN_REFS_NAMESPACE,
        ],
    )
    .map_err(|error| reclassify(&error, scope))?;
    if sandbox::inner_exit_code(&outcome) != Some(0) {
        return Err(unobservable_command(&outcome, scope));
    }

    let mut tips = BTreeMap::new();
    for line in outcome.stdout_text().lines() {
        let mut fields = line.split('\t');
        let (Some(reference), Some(commit), None) = (fields.next(), fields.next(), fields.next())
        else {
            return Ok(Err(UnobservableReason::AdvertisementInvalid));
        };
        if reference.is_empty() || commit.is_empty() {
            return Ok(Err(UnobservableReason::AdvertisementInvalid));
        }
        tips.insert(reference.to_string(), commit.to_string());
    }
    Ok(Ok(tips))
}

fn reaching_origin_refs(
    host: &dyn HostEnvironment,
    scope: &ObservationScope<'_>,
    commit: &str,
) -> Result<std::result::Result<BTreeSet<String>, UnobservableReason>> {
    let outcome = sandbox::exec(
        host,
        scope.sandbox,
        &[
            "git",
            "--git-dir",
            &scope.git_dir,
            "for-each-ref",
            "--format=%(refname)",
            &format!("--contains={commit}"),
            ORIGIN_REFS_NAMESPACE,
        ],
    )
    .map_err(|error| reclassify(&error, scope))?;
    match sandbox::inner_exit_code(&outcome) {
        Some(0) => {}
        Some(_) => return Ok(Err(UnobservableReason::ReadOnlyDataInsufficient)),
        None => return Err(unobservable_command(&outcome, scope)),
    }

    let mut origins = BTreeSet::new();
    for line in outcome.stdout_text().lines() {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Err(UnobservableReason::AdvertisementInvalid));
        }
        origins.insert(line.to_string());
    }
    Ok(Ok(origins))
}

fn object_present(
    host: &dyn HostEnvironment,
    scope: &ObservationScope<'_>,
    object: &str,
) -> Result<bool> {
    let outcome = sandbox::exec(
        host,
        scope.sandbox,
        &["git", "--git-dir", &scope.git_dir, "cat-file", "-e", object],
    )
    .map_err(|error| reclassify(&error, scope))?;
    match sandbox::inner_exit_code(&outcome) {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(unobservable_command(&outcome, scope)),
    }
}

fn reclassify(error: &Error, scope: &ObservationScope<'_>) -> Error {
    let mut diagnostic = observation_unobservable();
    if let Some(source) = error.diagnostics().first() {
        diagnostic.facts.clone_from(&source.facts);
        diagnostic.external.clone_from(&source.external);
    }
    diagnostic.facts.push(Fact::sandbox(scope.sandbox));
    Error::single(diagnostic)
}

fn unobservable_command(outcome: &CommandOutcome, scope: &ObservationScope<'_>) -> Error {
    Error::single(
        observation_unobservable()
            .fact(Fact::sandbox(scope.sandbox))
            .external(outcome.failure()),
    )
}

fn observation_unobservable() -> Diagnostic {
    Diagnostic::new(
        ErrorId::OriginObservationUnobservable,
        msg!("error-origin-observation-unobservable"),
    )
}

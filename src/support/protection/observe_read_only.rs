use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::boundary::host::{CommandOutcome, HostEnvironment};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::sandbox;

use super::{CommitCandidate, OriginObservation, UnobservableReason, add_saved};

const ORIGIN_REFS_NAMESPACE: &str = "refs/remotes/origin/";

/// 既にSandbox内にあるorigin refとobjectだけを使って、読み取り専用に観測する。
///
/// statusから呼ばれるため、originへfetchしない。local remote-tracking refが無い、
/// そのtip objectが無い、candidateのobject graphを検査できない、といった場合は
/// Unreachableへ丸めず、ReadOnlyDataInsufficientを返す。
///
/// 破壊操作の直前の観測と同じく、hostの`host_repository`へ保存済みの先端を`host:<ref>`
/// として足す。保存済みのcommitは、originへ届いていなくてもSandboxを消して失われない。
pub fn observe_read_only(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    layout: &SandboxLayout,
    candidates: &[CommitCandidate],
    host_repository: &Path,
) -> Result<OriginObservation> {
    let observation = observe_origin(host, sandbox, layout, candidates)?;
    Ok(add_saved(host, host_repository, sandbox, observation))
}

fn observe_origin(
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

    let unique_commits: BTreeSet<&str> = candidates.iter().map(CommitCandidate::commit).collect();
    // refとcandidateで同じobjectを何度も調べない。全objectをstdinで一度に渡し、
    // Sandboxへの往復とGitの起動を、ref数によらず1回にする。
    let objects: BTreeSet<&str> = tips
        .values()
        .map(String::as_str)
        .chain(unique_commits.iter().copied())
        .collect();
    if !objects_present(host, &scope, &objects)? {
        return Ok(unobservable(UnobservableReason::ReadOnlyDataInsufficient));
    }

    let mut reachable_from: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for commit in unique_commits {
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

fn objects_present(
    host: &dyn HostEnvironment,
    scope: &ObservationScope<'_>,
    objects: &BTreeSet<&str>,
) -> Result<bool> {
    // object ID以外をbatchの入力行として解釈させない。
    if objects.iter().any(|object| {
        !matches!(object.len(), 40 | 64) || !object.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err(crate::diagnostics::unparseable(
            "git cat-file --batch-check",
            "expected full hexadecimal object IDs",
        ));
    }
    let mut input = Vec::new();
    for object in objects {
        input.extend_from_slice(object.as_bytes());
        input.push(b'\n');
    }
    let outcome = sandbox::exec_with_input(
        host,
        scope.sandbox,
        &[
            "git",
            "--git-dir",
            &scope.git_dir,
            "cat-file",
            "--batch-check=%(objectname)",
        ],
        input,
    )
    .map_err(|error| reclassify(&error, scope))?;
    if sandbox::inner_exit_code(&outcome) != Some(0) {
        return Err(unobservable_command(&outcome, scope));
    }
    let output = outcome.stdout_text();
    let mut lines = output.lines();
    let mut present = true;
    for object in objects {
        match lines.next() {
            Some(line) if line == *object => {}
            Some(line) if line.strip_suffix(" missing") == Some(*object) => present = false,
            _ => {
                return Err(crate::diagnostics::unparseable(
                    "git cat-file --batch-check",
                    "missing, reordered, or invalid object response",
                ));
            }
        }
    }
    if lines.next().is_some() {
        return Err(crate::diagnostics::unparseable(
            "git cat-file --batch-check",
            "unexpected extra object response",
        ));
    }
    Ok(present)
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

#[cfg(test)]
#[path = "observe_read_only_test.rs"]
mod observe_read_only_test;

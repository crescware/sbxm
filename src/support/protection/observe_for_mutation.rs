use std::collections::{BTreeMap, BTreeSet};

use crate::command::{CommandOutcome, HostEnvironment};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::repository;
use crate::support::sandbox;

use super::{CommitCandidate, OriginObservation, UnobservableReason};

/// originのremote-tracking refを列挙するnamespace。
const ORIGIN_REFS_NAMESPACE: &str = "refs/remotes/origin/";

/// 破壊操作の直前に、originを権威ある状態としてrefreshしてから観測する。
///
/// 1. bare repositoryにoriginが設定されているかを確かめる。
/// 2. `fetch --prune`でremote-tracking refを最新化する。
/// 3. refresh後のorigin refを完全なref名とtip commit IDで列挙する。
/// 4. `candidates`が指すcommitごとに、どのorigin refから到達できるかを1回だけ求める。
///
/// Gitが正常に応答した結果として観測不能と判定した場合は
/// `OriginObservation::Unobservable`を返す。command自体を起動できない失敗は`Err`とする。
pub fn observe_for_mutation(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    layout: &SandboxLayout,
    project: &str,
    candidates: &[CommitCandidate],
) -> Result<OriginObservation> {
    let scope = ObservationScope {
        sandbox: sandbox.as_str(),
        git_dir: layout.bare_git_dir(),
        project,
    };

    if !origin_configured(host, &scope)? {
        return Ok(unobservable(UnobservableReason::OriginMissing));
    }

    let fetch = repository::refresh_origin(host, scope.sandbox, &scope.git_dir, None)
        .map_err(|error| reclassify(&error, &scope))?;
    match sandbox::inner_exit_code(&fetch) {
        Some(0) => {}
        Some(_) => return Ok(unobservable(UnobservableReason::RefreshFailed)),
        None => return Err(unobservable_command(&fetch, &scope)),
    }

    let tips = match origin_tips(host, &scope)? {
        Ok(tips) => tips,
        Err(reason) => return Ok(unobservable(reason)),
    };

    let mut reachable_from: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let unique_commits: BTreeSet<&str> = candidates.iter().map(CommitCandidate::commit).collect();
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

/// 1回の観測が使う、Sandboxとrepositoryの宛先。
struct ObservationScope<'a> {
    sandbox: &'a str,
    git_dir: String,
    project: &'a str,
}

fn unobservable(reason: UnobservableReason) -> OriginObservation {
    OriginObservation::Unobservable { reason }
}

/// bare repositoryに`origin`が設定されているか。
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
        // `git config --get`は、keyが無いことを`1`で示す。
        Some(1) => Ok(false),
        _ => Err(unobservable_command(&outcome, scope)),
    }
}

/// refresh後のorigin refを、完全なref名からtip commit IDへ写す。
///
/// 出力を解釈できなかった場合は`AdvertisementInvalid`を返す。
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

/// `commit`へ到達できるorigin ref名の集合。
///
/// `commit`がローカルのobject databaseに無ければ`ObjectMissing`を返す。
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
        Some(_) => return Ok(Err(UnobservableReason::ObjectMissing)),
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

/// origin観測commandの起動そのものに失敗した場合の共通の写像。
///
/// 元のdiagnosticが持つfactとexternal causeは、原因の説明として保持する。
fn reclassify(error: &Error, scope: &ObservationScope<'_>) -> Error {
    let mut diagnostic = observation_unobservable(scope);
    if let Some(source) = error.diagnostics().first() {
        diagnostic.facts.clone_from(&source.facts);
        diagnostic.external.clone_from(&source.external);
    }
    diagnostic.facts.push(Fact::sandbox(scope.sandbox));
    Error::single(diagnostic)
}

/// commandは起動できたが、終了statusが判定対象に無い場合。
fn unobservable_command(outcome: &CommandOutcome, scope: &ObservationScope<'_>) -> Error {
    Error::single(
        observation_unobservable(scope)
            .fact(Fact::sandbox(scope.sandbox))
            .external(outcome.failure()),
    )
}

/// originの観測そのものが成立しなかったことを表すdiagnosticの土台。
///
/// remote URLは持たせない。観測できなかった事実だけを示す。
fn observation_unobservable(scope: &ObservationScope<'_>) -> Diagnostic {
    Diagnostic::new(
        ErrorId::OriginObservationUnobservable,
        msg!("error-origin-observation-unobservable"),
    )
    .remediation(
        Remediation::text(msg!("remediation-origin-observation-unobservable"))
            .try_run(format!("sbxm open {}", scope.project)),
    )
}

#[cfg(test)]
#[path = "observe_for_mutation_test.rs"]
mod observe_for_mutation_test;

use std::collections::{BTreeMap, BTreeSet};

use crate::command::{CommandOutcome, HostEnvironment};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::repository;
use crate::support::sandbox;

use super::{CommitCandidate, OriginObservation, UnobservableReason};

/// originの権威ある広告を一時保存するnamespace。通常のremote-tracking refやlocal tagは
/// fetchで変更しない。
const OBSERVATION_REFS_NAMESPACE: &str = "refs/sbxm/origin/";

/// `OriginObservation`へ載せるbranchの表記。既存のupstream表記と一致させる。
const ORIGIN_REFS_NAMESPACE: &str = "refs/remotes/origin/";

/// 破壊操作の直前に、originを権威ある状態としてrefreshしてから観測する。
///
/// 1. bare repositoryにoriginが設定されているかを確かめる。
/// 2. `fetch --prune`でoriginが広告する全refを隔離namespaceへ最新化する。
/// 3. refresh後のorigin refを完全なref名とtip commit IDで列挙する。
/// 4. `candidates`が指すcommitごとに、どのorigin refから到達できるかを1回だけ求める。
///
/// Gitが正常に応答した結果として観測不能と判定した場合は
/// `OriginObservation::Unobservable`を返す。command自体を起動できない失敗は`Err`とする。
/// `Err`が持つdiagnosticにremediationは無い。projectを知らないためで、呼び出し側が
/// 案件固有のremediationを足す。
pub fn observe_for_mutation(
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

    // `--no-tags`でも明示的な`refs/*` refspecはtagを取得する。宛先は一時namespaceへ
    // 隔離するため、このfetchでlocalの`refs/tags/*`やremote-tracking refは変わらない。
    let fetch = repository::refresh_origin_all_refs(
        host,
        scope.sandbox,
        &scope.git_dir,
        OBSERVATION_REFS_NAMESPACE,
    )
    .map_err(|error| reclassify(&error, &scope))?;
    let fetch_status = sandbox::inner_exit_code(&fetch);
    if fetch_status != Some(0) {
        cleanup_temporary_refs(host, &scope)?;
        return match fetch_status {
            Some(_) => Ok(unobservable(UnobservableReason::RefreshFailed)),
            None => Err(unobservable_command(&fetch, &scope)),
        };
    }

    let observation = observe_temporary_refs(host, &scope, candidates);
    let cleanup = cleanup_temporary_refs(host, &scope);
    cleanup?;
    observation
}

fn observe_temporary_refs(
    host: &dyn HostEnvironment,
    scope: &ObservationScope<'_>,
    candidates: &[CommitCandidate],
) -> Result<OriginObservation> {
    let tips = match origin_tips(host, scope)? {
        Ok(tips) => tips,
        Err(reason) => return Ok(unobservable(reason)),
    };

    let mut reachable_from: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let unique_commits: BTreeSet<&str> = candidates.iter().map(CommitCandidate::commit).collect();
    for commit in unique_commits {
        match reaching_origin_refs(host, scope, commit)? {
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

/// 観測後に一時namespaceを確実に削除する。列挙できない場合も、残ったrefを無視して
/// 成功扱いにしない。
fn cleanup_temporary_refs(host: &dyn HostEnvironment, scope: &ObservationScope<'_>) -> Result<()> {
    let listed = sandbox::exec(
        host,
        scope.sandbox,
        &[
            "git",
            "--git-dir",
            &scope.git_dir,
            "for-each-ref",
            "--format=%(refname)",
            OBSERVATION_REFS_NAMESPACE,
        ],
    )
    .map_err(|error| reclassify(&error, scope))?;
    if sandbox::inner_exit_code(&listed) != Some(0) {
        return Err(unobservable_command(&listed, scope));
    }

    for reference in listed.stdout_text().lines() {
        if !reference.starts_with(OBSERVATION_REFS_NAMESPACE)
            || reference == OBSERVATION_REFS_NAMESPACE
        {
            return Err(unobservable_command(&listed, scope));
        }
        let deleted = sandbox::exec(
            host,
            scope.sandbox,
            &[
                "git",
                "--git-dir",
                &scope.git_dir,
                "update-ref",
                "-d",
                reference,
            ],
        )
        .map_err(|error| reclassify(&error, scope))?;
        if sandbox::inner_exit_code(&deleted) != Some(0) {
            return Err(unobservable_command(&deleted, scope));
        }
    }
    Ok(())
}

/// 1回の観測が使う、Sandboxとrepositoryの宛先。
struct ObservationScope<'a> {
    sandbox: &'a str,
    git_dir: String,
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
            OBSERVATION_REFS_NAMESPACE,
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
        let Some(reference) = observed_origin_ref(reference) else {
            return Ok(Err(UnobservableReason::AdvertisementInvalid));
        };
        if commit.is_empty() {
            return Ok(Err(UnobservableReason::AdvertisementInvalid));
        }
        if tips.insert(reference, commit.to_string()).is_some() {
            return Ok(Err(UnobservableReason::AdvertisementInvalid));
        }
    }
    Ok(Ok(tips))
}

/// `commit`へ到達できるorigin ref名の集合。
///
/// `commit`がローカルのobject databaseに無いと確かめられた場合だけ`ObjectMissing`を
/// 返す。確かめられない非ゼロ終了は、他の観測不能と同じく起動できたcommandの失敗
/// として`Err`にする。
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
            OBSERVATION_REFS_NAMESPACE,
        ],
    )
    .map_err(|error| reclassify(&error, scope))?;
    match sandbox::inner_exit_code(&outcome) {
        Some(0) => {}
        Some(_) => return classify_contains_failure(host, scope, commit, &outcome),
        None => return Err(unobservable_command(&outcome, scope)),
    }

    let mut origins = BTreeSet::new();
    for line in outcome.stdout_text().lines() {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Err(UnobservableReason::AdvertisementInvalid));
        }
        let Some(reference) = observed_origin_ref(line) else {
            return Ok(Err(UnobservableReason::AdvertisementInvalid));
        };
        origins.insert(reference);
    }
    Ok(Ok(origins))
}

/// 一時namespaceのrefを、既存のupstreamと一致するbranch名または広告された完全なref名へ
/// 戻す。branch以外は`refs/tags/*`や`refs/custom/*`のようなorigin側のnamespaceをそのまま
/// 保持する。
fn observed_origin_ref(reference: &str) -> Option<String> {
    let source = reference.strip_prefix(OBSERVATION_REFS_NAMESPACE)?;
    if let Some(branch) = source.strip_prefix("heads/") {
        return (!branch.is_empty()).then(|| format!("{ORIGIN_REFS_NAMESPACE}{branch}"));
    }
    (!source.is_empty()).then(|| format!("refs/{source}"))
}

/// `--contains`が非ゼロで終わった原因を確かめる。
///
/// `git cat-file -e`はobjectが無いことを`1`で示す。それだけが`ObjectMissing`の根拠で
/// あり、それ以外(objectはあるのに`--contains`が失敗した、`cat-file`自体を起動できない
/// 等)は、原因を断定せず`--contains`の失敗をそのまま観測不能な起動失敗として報告する。
fn classify_contains_failure(
    host: &dyn HostEnvironment,
    scope: &ObservationScope<'_>,
    commit: &str,
    contains_outcome: &CommandOutcome,
) -> Result<std::result::Result<BTreeSet<String>, UnobservableReason>> {
    let probe = sandbox::exec(
        host,
        scope.sandbox,
        &["git", "--git-dir", &scope.git_dir, "cat-file", "-e", commit],
    )
    .map_err(|error| reclassify(&error, scope))?;
    match sandbox::inner_exit_code(&probe) {
        Some(1) => Ok(Err(UnobservableReason::ObjectMissing)),
        _ => Err(unobservable_command(contains_outcome, scope)),
    }
}

/// origin観測commandの起動そのものに失敗した場合の共通の写像。
///
/// 元のdiagnosticが持つfactとexternal causeは、原因の説明として保持する。
fn reclassify(error: &Error, scope: &ObservationScope<'_>) -> Error {
    let mut diagnostic = observation_unobservable();
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
        observation_unobservable()
            .fact(Fact::sandbox(scope.sandbox))
            .external(outcome.failure()),
    )
}

/// originの観測そのものが成立しなかったことを表すdiagnosticの土台。
///
/// remote URLは持たせない。観測できなかった事実だけを示す。remediationは持たない。
/// この関数はprojectを知らないため、呼び出し側が足す。
fn observation_unobservable() -> Diagnostic {
    Diagnostic::new(
        ErrorId::OriginObservationUnobservable,
        msg!("error-origin-observation-unobservable"),
    )
}

#[cfg(test)]
#[path = "observe_for_mutation_test.rs"]
mod observe_for_mutation_test;

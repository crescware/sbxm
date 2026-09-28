use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::{Result, unparseable};
use crate::support::repository::{host_git, refusal_reason};

use super::{ReflectResult, Reflected, move_checked_out, saved_namespace};

/// Sandboxから保存したbranchとtagを、hostの`repository`のbranchとtagへ反映する。
///
/// hostのrepositoryが自分自身へ、`refs/sbx/<sandbox>/heads/*`と`tags/*`をpushする。受け取る
/// git（receive-pack）がrefごとに判断するため、早送りでない更新と、指す先の違う同じ名前の
/// tagは、gitとhostのrepositoryの設定に従って断られる。消えたbranchやtagは運ばない。
///
/// hostでcheckoutしているbranchだけは、pushでは動かさず、そのworktreeで早送りする
/// （`move_checked_out`）。pushに任せると、hostのrepositoryの`receive.denyCurrentBranch`
/// しだいで、断られるか、未commitの変更が1つでもあると断られるか、作業treeを残して
/// refだけが進む。そのためpushでは、設定によらずcheckout中のbranchを断らせる。
///
/// 受け取る側のhookは、hostのrepositoryの規則として走らせる。送る側の`pre-push`は、別の
/// repositoryへ送る前の確認であり、自分自身へ反映するときには走らせない。
///
/// 変わらなかったrefは返さない。
pub fn reflect_saved(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &str,
) -> Result<Vec<Reflected>> {
    let namespace = saved_namespace(sandbox);
    let heads = format!("{namespace}heads/*:refs/heads/*");
    let tags = format!("{namespace}tags/*:refs/tags/*");
    let pushed = host_git(
        host,
        repository,
        &[
            "push",
            "--porcelain",
            "--no-verify",
            RECEIVE_PACK,
            ".",
            &heads,
            &tags,
        ],
        None,
        // hostのrepositoryのreceive hookが、このpushの中で走る。手元のfileの読み書きだけ
        // では終わらない。
        TimeoutClass::RepositoryTransfer,
    )?;
    // 断ったrefがあれば1で終わる。それ以外の失敗は、refごとの答えを持たない。
    let pushed = if matches!(pushed.status.code(), Some(0 | 1)) {
        pushed
    } else {
        pushed.require_success()?
    };
    let mut reflected = Vec::new();
    let mut rejected = false;
    for line in pushed.stdout_text().lines() {
        let mut fields = line.split('\t');
        let (Some(flag), Some(refspec), Some(summary)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some((source, reference)) = refspec.split_once(':') else {
            return Err(unparseable(
                "git push",
                "a ref line had no source and destination",
            ));
        };
        let result = match flag {
            " " | "+" => ReflectResult::Updated,
            "*" => ReflectResult::Created,
            "=" | "-" => continue,
            "!" => {
                rejected = true;
                rejection(host, repository, source, reference, summary)?
            }
            _ => return Err(unparseable("git push", "a ref line had an unknown flag")),
        };
        reflected.push(Reflected {
            reference: reference.to_string(),
            result,
        });
    }
    // 1で終わったのに断ったrefを1つも読めない失敗は、refごとの答えではない。何も
    // 反映しなかった成功とは読まない。
    if !rejected && !pushed.success() {
        pushed.require_success()?;
    }
    Ok(reflected)
}

/// 受け取る側のgit。`git -c`で渡した設定は、同じ機械の中のpushでも受け取る側へ届かない。
const RECEIVE_PACK: &str = "--receive-pack=git -c receive.denyCurrentBranch=refuse receive-pack";

/// 断られたrefの理由。早送りでないものは、Sandboxが遅れているだけかを見分ける。
/// checkout中で断られたbranchは、そのworktreeで早送りする。
fn rejection(
    host: &dyn HostEnvironment,
    repository: &Path,
    source: &str,
    reference: &str,
    summary: &str,
) -> Result<ReflectResult> {
    let reason = refusal_reason(summary);
    Ok(match reason {
        "non-fast-forward" if contains(host, repository, reference, source)? => {
            ReflectResult::Behind
        }
        "non-fast-forward" => ReflectResult::Diverged,
        "branch is currently checked out" => move_checked_out(host, repository, source, reference)?,
        "already exists" => ReflectResult::Exists,
        _ => ReflectResult::Refused {
            reason: reason.to_string(),
        },
    })
}

/// `reference`が`source`の先端を含むか。判定できない場合は含まないとする。
fn contains(
    host: &dyn HostEnvironment,
    repository: &Path,
    reference: &str,
    source: &str,
) -> Result<bool> {
    let outcome = host_git(
        host,
        repository,
        &["merge-base", "--is-ancestor", source, reference],
        None,
        TimeoutClass::LocalFilesystem,
    )?;
    Ok(outcome.status.code() == Some(0))
}

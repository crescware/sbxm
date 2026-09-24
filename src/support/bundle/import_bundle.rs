use std::collections::BTreeMap;
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::paths;
use crate::support::repository::host_git;

use super::{REF_KINDS, RefChange, saved_namespace, saved_refs};

/// bundleを、hostの`repository`の`refs/sbx/<namespace>/`へ取り込む。
///
/// hostのbranchやtagには触れない。bundleはobjectを確かめながら一時的な名前空間へ
/// 取り込み、そこから名前空間の中のrefを書き換える。早送りでない更新と、Sandboxに
/// 無くなったrefは、前の先端を`archive/<label>/`へ退避してから書き換え、退避した
/// refは消さない。一度取り込んだcommitは、どれかのrefから辿れ続ける。
///
/// 書き換えは2回のtransactionに分ける。先に前の先端の退避と消えたrefの削除を、次に
/// 更新と作成を行う。`foo`から`foo/bar`への改名のように、消す名前と作る名前が
/// fileとdirectoryで重なっても、同じtransactionの中でぶつからない。先の分だけ
/// 済んだ場合も、前の先端は退避済みであり、次の取り込みが残りを作る。
///
/// Sandboxのrefの名前は種類ごとに決めた場所へだけ写す。`archive/`へ届く写し方は無い。
pub fn import_bundle(
    host: &dyn HostEnvironment,
    repository: &Path,
    bundle: &Path,
    namespace: &str,
    label: &str,
) -> Result<Vec<RefChange>> {
    let bundle = paths::display(bundle);
    host_git(
        host,
        repository,
        &["bundle", "verify", "--quiet", &bundle],
        None,
        TimeoutClass::RepositoryTransfer,
    )?
    .require_success()?;

    let incoming = format!("refs/sbx-incoming/{namespace}/");
    clear(host, repository, &incoming)?;
    let imported = import_through(host, repository, &bundle, &incoming, namespace, label);
    // 一時的な名前空間は、取り込めても取り込めなくても残さない。
    let cleared = clear(host, repository, &incoming);
    let changes = imported?;
    cleared?;
    Ok(changes)
}

fn import_through(
    host: &dyn HostEnvironment,
    repository: &Path,
    bundle: &str,
    incoming: &str,
    namespace: &str,
    label: &str,
) -> Result<Vec<RefChange>> {
    let destination = saved_namespace(namespace);
    let refspecs: Vec<String> = REF_KINDS
        .iter()
        .map(|(source, kind)| format!("+{source}*:{incoming}{kind}*"))
        .collect();
    let mut args = vec![
        "-c",
        "transfer.fsckObjects=true",
        "fetch",
        "--no-tags",
        "--no-write-fetch-head",
        "--quiet",
        bundle,
    ];
    args.extend(refspecs.iter().map(String::as_str));
    host_git(
        host,
        repository,
        &args,
        None,
        TimeoutClass::RepositoryTransfer,
    )?
    .require_success()?;

    let arrived = refs_under(host, repository, incoming)?;
    let existing = saved_refs(host, repository, namespace)?;

    // 先に退避と削除を、次に更新と作成を行う。
    let mut retire = Vec::new();
    let mut advance = Vec::new();
    let mut changes = Vec::new();
    for (name, old) in &existing {
        let reference = format!("{destination}{name}");
        let archived = format!("{destination}archive/{label}/{name}");
        match arrived.get(name) {
            Some(new) if new == old => {}
            Some(new) if fast_forward(host, repository, old, new)? => {
                advance.push(format!("update {reference} {new} {old}"));
                changes.push(RefChange::Updated { reference });
            }
            Some(new) => {
                retire.push(format!("create {archived} {old}"));
                advance.push(format!("update {reference} {new} {old}"));
                changes.push(RefChange::Replaced {
                    reference,
                    archived,
                });
            }
            None => {
                retire.push(format!("create {archived} {old}"));
                retire.push(format!("delete {reference} {old}"));
                changes.push(RefChange::Deleted {
                    reference,
                    archived,
                });
            }
        }
    }
    for (name, new) in &arrived {
        if !existing.contains_key(name) {
            let reference = format!("{destination}{name}");
            advance.push(format!("create {reference} {new}"));
            changes.push(RefChange::Created { reference });
        }
    }
    transaction(host, repository, retire)?;
    transaction(host, repository, advance)?;
    changes.sort_by(|left, right| left.reference().cmp(right.reference()));
    Ok(changes)
}

/// `commands`を1回の`update-ref`のtransactionで行う。空なら何もしない。
fn transaction(host: &dyn HostEnvironment, repository: &Path, commands: Vec<String>) -> Result<()> {
    if commands.is_empty() {
        return Ok(());
    }
    let mut input = String::from("start\n");
    for command in commands {
        input.push_str(&command);
        input.push('\n');
    }
    input.push_str("commit\n");
    host_git(
        host,
        repository,
        &["update-ref", "--stdin"],
        Some(input.into_bytes()),
        TimeoutClass::LocalFilesystem,
    )?
    .require_success()?;
    Ok(())
}

/// `prefix`の下にあるref。名前は`prefix`を除いた残りで返す。
fn refs_under(
    host: &dyn HostEnvironment,
    repository: &Path,
    prefix: &str,
) -> Result<BTreeMap<String, String>> {
    let listed = host_git(
        host,
        repository,
        &["for-each-ref", "--format=%(objectname) %(refname)", prefix],
        None,
        TimeoutClass::LocalFilesystem,
    )?
    .require_success()?
    .stdout_text();
    Ok(listed
        .lines()
        .filter_map(|line| line.split_once(' '))
        .filter_map(|(tip, reference)| {
            reference
                .strip_prefix(prefix)
                .map(|name| (name.to_string(), tip.to_string()))
        })
        .collect())
}

/// `old`から`new`へ早送りできるか。判定できない場合も、失わない側へ倒して`false`とする。
fn fast_forward(
    host: &dyn HostEnvironment,
    repository: &Path,
    old: &str,
    new: &str,
) -> Result<bool> {
    let outcome = host_git(
        host,
        repository,
        &["merge-base", "--is-ancestor", old, new],
        None,
        TimeoutClass::LocalFilesystem,
    )?;
    Ok(outcome.status.code() == Some(0))
}

/// `prefix`の下のrefをすべて消す。
fn clear(host: &dyn HostEnvironment, repository: &Path, prefix: &str) -> Result<()> {
    let names = refs_under(host, repository, prefix)?;
    transaction(
        host,
        repository,
        names
            .keys()
            .map(|name| format!("delete {prefix}{name}"))
            .collect(),
    )
}

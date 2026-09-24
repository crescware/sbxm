use std::collections::BTreeMap;
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::support::repository::host_git;

use super::{REF_KINDS, saved_namespace};

/// hostの`repository`が`sandbox`のために保存している、種類ごとのref。
///
/// 名前は名前空間を除いた`heads/<branch>`のような形で返す。退避した`archive/`は、どの
/// 種類の下にも無いため含まない。種類の下にある`archive/`で始まる名前は、Sandboxの
/// branchやtagであり、ほかのrefと同じく突き合わせる。
pub(super) fn saved_refs(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &str,
) -> Result<BTreeMap<String, String>> {
    let namespace = saved_namespace(sandbox);
    let mut saved = BTreeMap::new();
    for (_, kind) in REF_KINDS {
        let prefix = format!("{namespace}{kind}");
        let listed = host_git(
            host,
            repository,
            &["for-each-ref", "--format=%(objectname) %(refname)", &prefix],
            None,
            TimeoutClass::LocalFilesystem,
        )?
        .require_success()?
        .stdout_text();
        saved.extend(listed.lines().filter_map(|line| {
            let (tip, reference) = line.split_once(' ')?;
            let name = reference.strip_prefix(&prefix)?;
            Some((format!("{kind}{name}"), tip.to_string()))
        }));
    }
    Ok(saved)
}

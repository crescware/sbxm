use std::path::Path;
use std::time::SystemTime;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::SandboxName;

use super::{RefChange, finish_save, import_from_sandbox, prepare_save, stamp};

/// Sandboxの`git_dir`のcommitを、hostの`repository`の`refs/sbx/<sandbox>/`へ保存する。
///
/// hostのgitがssh越しにSandboxのrepositoryをfetchする。gitは足りないobjectだけを
/// 運ぶため、何も変えていないSandboxの保存は、何も運ばずに空の変更を返す。Sandboxが
/// 保存するrefを1つも持たなければ、保存済みのrefに触れずに`None`を返す。Sandboxは
/// 動いていることを呼び出し側が確かめておく。
pub fn save_to_host(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    git_dir: &str,
    repository: &Path,
) -> Result<Option<Vec<RefChange>>> {
    if !prepare_save(host, sandbox.as_str(), git_dir)? {
        return Ok(None);
    }
    let imported = import_from_sandbox(
        host,
        repository,
        sandbox.as_str(),
        git_dir,
        &stamp(SystemTime::now()),
    );
    // Sandboxの一時refは、取り込めても取り込めなくても残さない。
    let cleared = finish_save(host, sandbox.as_str(), git_dir);
    let changes = imported?;
    cleared?;
    Ok(Some(changes))
}

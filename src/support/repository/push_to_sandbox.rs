use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::{Result, unparseable};

use super::{PushRefusal, host_git, sandbox_remote, sandbox_ssh_config, sandbox_unwritable};

/// hostの`repository`のbranchとtagを、`sandbox`のoriginへpushする。gitが断ったrefを返す。
///
/// Sandboxの中で`git fetch --prune origin`をしたときと同じものを、hostから書き込む。
/// branchは`refs/remotes/origin/*`へ強制して置き、hostで消したbranchはSandboxからも
/// 消す。tagは同じ名前へ置き、上書きしない。Sandboxで作ったtagを、hostに無いことを
/// 理由に消さないよう、tagは別のpushで`--prune`せずに送る。
///
/// 送る側の`pre-push`は、hostのrepositoryから外へ出すときの確認であり、sbxmがSandboxの
/// originを最新にするときには走らせない。
pub fn push_to_sandbox(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &str,
    git_dir: &str,
) -> Result<Vec<PushRefusal>> {
    let mut refused = push(
        host,
        repository,
        sandbox,
        git_dir,
        &["--prune", "+refs/heads/*:refs/remotes/origin/*"],
    )?;
    refused.extend(push(
        host,
        repository,
        sandbox,
        git_dir,
        &["refs/tags/*:refs/tags/*"],
    )?);
    Ok(refused)
}

fn push(
    host: &dyn HostEnvironment,
    repository: &Path,
    sandbox: &str,
    git_dir: &str,
    tail: &[&str],
) -> Result<Vec<PushRefusal>> {
    let remote = sandbox_remote(sandbox, git_dir);
    let ssh = sandbox_ssh_config();
    let mut args = vec![
        "-c",
        ssh.as_str(),
        "push",
        "--porcelain",
        "--no-verify",
        remote.as_str(),
    ];
    args.extend_from_slice(tail);
    let pushed = host_git(
        host,
        repository,
        &args,
        None,
        TimeoutClass::RepositoryTransfer,
    )?;
    // 断ったrefがあれば1で終わる。それ以外の失敗は、refごとの答えを持たない。
    if !matches!(pushed.status.code(), Some(0 | 1)) {
        return Err(sandbox_unwritable(sandbox, &pushed));
    }
    let mut refused = Vec::new();
    for line in pushed.stdout_text().lines() {
        let mut fields = line.split('\t');
        let (Some("!"), Some(refspec), Some(summary)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some((_, reference)) = refspec.split_once(':') else {
            return Err(unparseable(
                "git push",
                "a ref line had no source and destination",
            ));
        };
        let reason = summary
            .rsplit_once(" (")
            .and_then(|(_, reason)| reason.strip_suffix(')'))
            .unwrap_or(summary);
        refused.push(PushRefusal {
            reference: reference.to_string(),
            reason: reason.to_string(),
        });
    }
    // 断ったrefが1つも読めない失敗は、refごとの答えではない。
    if refused.is_empty() && !pushed.success() {
        return Err(sandbox_unwritable(sandbox, &pushed));
    }
    Ok(refused)
}

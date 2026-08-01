use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::msg;

use crate::support::sandbox;

use crate::support::repository::unusable;

/// 既に案件の成果物として記録済みのworktreeを、そのまま引き継ぐ。
///
/// 求めるのは、この共有repositoryのworktreeであり続けていることだけとする。
///
/// 起点commitもmodeも条件にしない。そこで作業するためのworktreeであり、commitすれば
/// HEADは動き、branchを切ればmodeも変わる。そこで起きたことを異常として扱うと、
/// 作業した案件はworktreeを増やせなくなる。どちらもsbxmが作るときの事後条件であって、
/// 既にあるものへの要件ではない。
pub fn adopt_worktree(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    path: &str,
) -> Result<()> {
    // `--path-format=absolute`を付けないと、gitは条件によって相対pathを返す。bare git
    // dirとの一致を見る比較では、返る形が決まっていないと判定にならない。
    let common = sandbox::read(
        host,
        sandbox,
        &[
            "git",
            "-C",
            path,
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ],
    )?;
    if common != git_dir {
        return Err(unusable(
            path,
            msg!(
                "cause-worktree-belongs-elsewhere",
                observed = common,
                expected = git_dir
            ),
        ));
    }
    Ok(())
}

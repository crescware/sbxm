/// bareリポジトリの有無を確かめる、Sandbox内で走らせるshell script。
///
/// mount元が消えたSandboxへの`sbx exec`は、内側のshellを起動できないまま終了status
/// だけを返す。その終了statusは`test -e`が答える`0`/`1`と重なり、終了statusだけでは
/// 「repositoryが無い」と「観測できない」を区別できない(host側で直前に確かめても、
/// その確認とこの`sbx exec`の間にも消えうる)。内側のshellが実際に走った場合だけ、
/// `test`の前に印をstdoutへ書く。印の無いstdoutは、終了statusを`test`の答えとして
/// 読まない理由になる。
pub(crate) const BARE_GIT_DIR_PROBE: &str = "printf probed; test -e \"$1\"";

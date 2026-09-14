/// Docker Sandboxesが組み込みで宣言する、GitHub向けのservice識別子。
///
/// このserviceはSandboxの`GH_TOKEN`と`GITHUB_TOKEN`をsentinelで埋め、github.com、
/// api.github.com、raw.githubusercontent.comなどへのrequestでsentinelを本物へ
/// 差し替える。対象hostの一覧はDocker Sandboxes側が持ち、sbxmは繰り返さない。
pub const GITHUB_SERVICE: &str = "github";

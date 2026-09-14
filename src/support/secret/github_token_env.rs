/// 登録が`--env`で埋めるSandbox内の環境変数名。
///
/// sbxm自身はこの変数を読まない。placeholderはcredential helperが直接持つ。
/// Docker Sandboxesがこの名前を組み込み`github` serviceのために予約しており、
/// custom secretの値がSandboxへ届く保証がないためである。Sandboxの中で`gh`などが
/// 読めるよう、登録commandには残す。
pub const GITHUB_TOKEN_ENV: &str = "GH_TOKEN";

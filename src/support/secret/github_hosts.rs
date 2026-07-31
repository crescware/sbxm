use super::GITHUB_HOST;

/// proxyが認証を差し替える対象host。
///
/// 開発中にtokenを提示する先をすべて覆う。登録のないhostへはplaceholderがそのまま
/// 送られ、tokenが正しくても認証されない。gitがgithub.comで通る一方、`gh`が
/// api.github.comで401になっていたのがこれである。
///
/// `sbx secret set-custom --host`はwildcardを受け取る。`*`が1 label、`**`が任意個の
/// labelに一致する。個別のhostを並べるより、subdomainが増えても追随する形を選ぶ。
/// この4件が覆う範囲:
///
/// - `**.github.com`: api（gh、REST、GraphQL）、codeload（tarball、`go get`）、
///   uploads（release asset、添付）、`*.pkg.github.com`（GitHub Packages）、gist
/// - `**.githubusercontent.com`: raw（private repositoryのfile）ほか
///
/// 全hostが1件のcustom secretに載っている必要がある。secretを分けるとplaceholderも
/// 分かれるが、Sandboxの`GH_TOKEN`は1つの値しか持てない。
///
/// ここに並べる文字列は、利用者が実行するcommandへそのまま入り、`sbx secret ls`の
/// `TARGETS`と文字列として突き合わせる。展開した結果ではなく、書いたとおりを比べる。
pub const GITHUB_HOSTS: [&str; 4] = [
    // git clone / fetch / push
    GITHUB_HOST,
    "**.github.com",
    "**.githubusercontent.com",
    // GitHub Container Index
    "ghcr.io",
];

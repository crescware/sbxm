use super::require_github;

/// remote `URLを比較用のcanonical` project IDへ正規化する。
///
/// 同じrepositoryを指すSSHとHTTPSの表記を同じ値へ寄せる。GitHub以外を指すURLと、
/// `<owner>/<repository>`を読み取れないURLは`None`とする。
pub fn canonical_id_of_remote(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = if let Some(rest) = url.strip_prefix("git@") {
        // git@github.com:owner/repository.git
        let (host, path) = rest.split_once(':')?;
        require_github(host)?;
        path
    } else {
        let rest = url
            .strip_prefix("ssh://git@")
            .or_else(|| url.strip_prefix("ssh://"))
            .or_else(|| url.strip_prefix("https://"))
            .or_else(|| url.strip_prefix("http://"))
            .or_else(|| url.strip_prefix("git://"))?;
        let (authority, path) = rest.split_once('/')?;
        // ssh://git@github.com:22/owner/repository.git
        let host = authority.rsplit('@').next()?;
        let host = host.split(':').next()?;
        require_github(host)?;
        path
    };

    let path = rest.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repository) = path.split_once('/')?;
    if owner.is_empty() || repository.is_empty() || repository.contains('/') {
        return None;
    }
    Some(format!(
        "{}/{}",
        owner.to_ascii_lowercase(),
        repository.to_ascii_lowercase()
    ))
}

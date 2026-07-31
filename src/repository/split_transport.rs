use super::{CloneTransport, SSH_USER, require_github};

/// transportを決め、`<owner>/<repository>.git`にあたる部分を返す。
///
/// `ssh://`と`http://`は受理しない。HTTPSはcredential、port、query、fragmentを
/// 持たないものだけを受理する。
pub(super) fn split_transport(value: &str) -> Option<(CloneTransport, &str)> {
    if let Some(rest) = value.strip_prefix(&format!("{SSH_USER}@")) {
        let (host, path) = rest.split_once(':')?;
        require_github(host)?;
        return Some((CloneTransport::Ssh, path));
    }

    let rest = value.strip_prefix("https://")?;
    let (authority, path) = rest.split_once('/')?;
    // credentialとportを持つauthorityは受理しない。
    if authority.contains('@') || authority.contains(':') {
        return None;
    }
    require_github(authority)?;
    if path.contains('?') || path.contains('#') {
        return None;
    }
    Some((CloneTransport::Https, path))
}

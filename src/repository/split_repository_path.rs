use super::GIT_SUFFIX;

/// `<owner>/<repository>.git`だけを受理し、ownerとrepositoryへ分ける。
pub(super) fn split_repository_path(path: &str) -> Option<(&str, &str)> {
    let path = path.strip_suffix(GIT_SUFFIX)?;
    let mut parts = path.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some((owner, name))
}

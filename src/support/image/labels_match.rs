use crate::compatibility::ImageIdentity;

pub(super) fn labels_match(identity: &ImageIdentity, expected: &[(String, String)]) -> bool {
    expected
        .iter()
        .all(|(key, value)| identity.labels.get(key) == Some(value))
}

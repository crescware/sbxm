use crate::boundary::host::protocol::ImageIdentity;

pub fn labels_match(identity: &ImageIdentity, expected: &[(String, String)]) -> bool {
    expected
        .iter()
        .all(|(key, value)| identity.labels.get(key) == Some(value))
}

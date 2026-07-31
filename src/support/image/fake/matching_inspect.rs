use crate::testing::outcome::Checked;

use super::{declared_labels, inspect_output};

pub fn matching_inspect() -> Checked<String> {
    let owned = declared_labels()?;
    let borrowed: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    Ok(inspect_output(&borrowed))
}

use crate::testing::outcome::{Checked, Required};

use crate::commands::status::project::{ProjectStatus, Value};

pub fn value_of(status: &ProjectStatus, item: &str) -> Checked<Value> {
    Ok(status
        .items
        .iter()
        .find(|entry| entry.item == item)
        .required_because(&format!("item {item} is missing"))?
        .value)
}

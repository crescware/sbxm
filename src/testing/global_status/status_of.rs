use crate::testing::outcome::{Checked, Required};

use crate::commands::status::global::GlobalStatus;
use crate::support::StatusValue;

pub fn status_of(status: &GlobalStatus, item: &str) -> Checked<StatusValue> {
    Ok(status
        .rows
        .iter()
        .find(|row| row.item == item)
        .required_because(&format!("row {item} is missing"))?
        .status)
}

use crate::commands::status::global::GlobalStatus;

pub fn items(status: &GlobalStatus) -> Vec<&'static str> {
    status.rows.iter().map(|row| row.item).collect()
}

use crate::support::{Row, StatusValue};

use super::GlobalStatus;

pub(super) fn push(status: &mut GlobalStatus, item: &'static str, value: StatusValue) {
    status.rows.push(Row {
        item,
        status: value,
    });
}

use crate::diagnostics::{Error, Result};
use crate::metadata::ProjectMetadata;
use crate::support::provisioning::{NextAction, Observation};

use super::{ProjectStatus, Value};

/// 次の1手を決める追加観測が失敗しても、既に読めた結果と失敗理由を残す。
pub(super) fn check_next(
    metadata: &ProjectMetadata,
    observed: Result<Observation>,
    status: &mut ProjectStatus,
) -> Result<()> {
    status.next = None;
    match observed {
        Ok(observation) => {
            for error in observation.blocking_errors() {
                append_error(status, error)?;
            }
            if !observation.blocking_errors().is_empty() {
                status.push("status-item-maintenance-check", Value::NotObserved);
            }
            status.next = NextAction::decide(metadata, &observation);
        }
        Err(error) => {
            append_error(status, &error)?;
            status.push("status-item-maintenance-check", Value::NotObserved);
        }
    }
    Ok(())
}

fn append_error(status: &mut ProjectStatus, error: &Error) -> Result<()> {
    if *error == Error::Canceled {
        return Err(Error::Canceled);
    }
    for diagnostic in error.diagnostics() {
        if !status.diagnostics.contains(diagnostic) {
            status.diagnostics.push(diagnostic.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "check_next_test.rs"]
mod check_next_test;

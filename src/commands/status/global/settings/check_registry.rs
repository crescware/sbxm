use crate::config::ConfigLocation;
use crate::registry;

use crate::support::StatusValue;

use crate::commands::status::global::{GlobalStatus, push};

/// registry documentのversion、構文、permission、不変条件。
///
/// 不在は登録案件0件として正常に扱う。個々の案件へは触れない。
pub fn check_registry(location: &ConfigLocation, status: &mut GlobalStatus) {
    let value = match registry::load(location) {
        Ok(registry) if registry.entries().is_empty() => StatusValue::Missing,
        Ok(_) => StatusValue::Ready,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            StatusValue::Error
        }
    };
    push(status, "status-item-registry", value);
}

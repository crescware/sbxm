use crate::diagnostics::Error;

pub fn describe(error: &Error) -> String {
    error.diagnostics().first().map_or_else(
        || "canceled".to_string(),
        |diagnostic| diagnostic.id.to_string(),
    )
}

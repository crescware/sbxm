use crate::diagnostics::Error;

pub fn external_of(error: &Error) -> Option<crate::diagnostics::ExternalFailure> {
    error
        .diagnostics()
        .first()
        .and_then(|diagnostic| diagnostic.external.clone())
}

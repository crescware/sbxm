use clap::error::{ContextKind, ContextValue};

pub(super) fn context_string(error: &clap::Error, kind: ContextKind) -> Option<String> {
    match error.get(kind) {
        Some(ContextValue::String(value)) => Some(value.clone()),
        Some(ContextValue::Strings(values)) => Some(values.join(", ")),
        Some(ContextValue::StyledStr(value)) => Some(value.to_string()),
        Some(ContextValue::StyledStrs(values)) => Some(
            values
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        ),
        _ => None,
    }
}

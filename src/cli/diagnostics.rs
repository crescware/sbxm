//! CLI parser libraryのerrorを、翻訳した説明と安定したerror IDへ写像する。

use clap::error::{ContextKind, ContextValue, ErrorKind};

use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::Outcome;

pub fn interpret(error: clap::Error) -> Result<Outcome> {
    match error.kind() {
        // helpとversionはexit code `0`。libraryの既定exit codeは透過しない。
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            Ok(Outcome::Help(error.render().to_string()))
        }
        ErrorKind::DisplayVersion => Ok(Outcome::Version(super::version_line())),
        _ => Err(map(&error)),
    }
}

fn map(error: &clap::Error) -> Error {
    let invalid_arg = context_string(error, ContextKind::InvalidArg);
    let invalid_value = context_string(error, ContextKind::InvalidValue);
    let usage = context_string(error, ContextKind::Usage);

    let (id, description) = match error.kind() {
        ErrorKind::UnknownArgument => (
            ErrorId::UnknownArgument,
            msg!(
                "error-unknown-argument",
                argument = invalid_arg.clone().unwrap_or_default()
            ),
        ),
        ErrorKind::InvalidValue | ErrorKind::ValueValidation => (
            ErrorId::InvalidValue,
            msg!(
                "error-invalid-value",
                argument = invalid_arg.clone().unwrap_or_default(),
                value = invalid_value.clone().unwrap_or_default()
            ),
        ),
        ErrorKind::InvalidSubcommand => (
            ErrorId::UnknownSubcommand,
            msg!(
                "error-unknown-subcommand",
                subcommand =
                    context_string(error, ContextKind::InvalidSubcommand).unwrap_or_default()
            ),
        ),
        ErrorKind::MissingRequiredArgument => (
            ErrorId::MissingRequiredArgument,
            msg!(
                "error-missing-required-argument",
                argument = context_string(error, ContextKind::InvalidArg).unwrap_or_default()
            ),
        ),
        ErrorKind::MissingSubcommand => {
            (ErrorId::MissingSubcommand, msg!("error-missing-subcommand"))
        }
        ErrorKind::ArgumentConflict => (
            ErrorId::ConflictingArguments,
            msg!(
                "error-conflicting-arguments",
                arguments = context_string(error, ContextKind::PriorArg)
                    .map(|prior| match &invalid_arg {
                        Some(invalid) => format!("{invalid}, {prior}"),
                        None => prior,
                    })
                    .or_else(|| invalid_arg.clone())
                    .unwrap_or_default()
            ),
        ),
        _ => (ErrorId::InvalidArguments, msg!("error-invalid-arguments")),
    };

    let mut diagnostic = Diagnostic::new(id, description);
    if let Some(usage) = usage {
        diagnostic = diagnostic.remediation(msg!("usage-hint", usage = usage.trim()));
    } else {
        diagnostic = diagnostic.remediation(msg!("remediation-run-help", command = "sbxm --help"));
    }
    Error::single(diagnostic)
}

fn context_string(error: &clap::Error, kind: ContextKind) -> Option<String> {
    match error.get(kind) {
        Some(ContextValue::String(value)) => Some(value.clone()),
        Some(ContextValue::Strings(values)) => Some(values.join(", ")),
        Some(ContextValue::StyledStr(value)) => Some(value.to_string()),
        Some(ContextValue::StyledStrs(values)) => Some(
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        _ => None,
    }
}

use crate::diagnostics::{ErrorId, Result, fail};
use crate::i18n::{Catalog, Locale};
use crate::msg;

/// `guide`が案内できる利用者の目的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    CredentialRotation,
}

impl Topic {
    pub const ALL: [Topic; 1] = [Topic::CredentialRotation];

    pub fn parse(value: &str) -> Result<Topic> {
        match value {
            "credential-rotation" => Ok(Topic::CredentialRotation),
            _ => fail(
                ErrorId::InvalidValue,
                msg!("error-invalid-value", argument = "<topic>", value = value),
            ),
        }
    }

    pub fn label(self, locale: Locale) -> String {
        let id = match self {
            Topic::CredentialRotation => "guide-topic-credential-rotation",
        };
        Catalog::new(locale)
            .text(id)
            .unwrap_or_else(|failure| failure.to_string())
    }
}

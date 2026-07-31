use crate::diagnostics::{Error, ErrorId, Result, fail};
use crate::msg;

use super::CanonicalProjectId;

/// `<owner>/<repository>`。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectId {
    value: String,
    /// `value`の中のslashのbyte位置。`parse`が数えた1個だけがここに入る。
    slash: usize,
}

impl ProjectId {
    /// 1個のslashを持つ`<owner>/<repository>`を検証する。
    pub fn parse(value: &str) -> Result<ProjectId> {
        let invalid = || {
            Error::new(
                ErrorId::InvalidProjectId,
                msg!("error-invalid-project-id", value = value),
            )
        };

        let mut parts = value.split('/');
        let (Some(owner), Some(repository), None) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid());
        };

        if !is_valid_owner(owner) || !is_valid_repository(repository) {
            return Err(invalid());
        }
        if repository == "." || repository == ".." {
            return Err(invalid());
        }
        // `.sbxm`はsbxmが予約する名前である。
        if repository.eq_ignore_ascii_case(".sbxm") {
            return fail(
                ErrorId::ReservedRepositoryName,
                msg!("error-reserved-repository-name", value = repository),
            );
        }

        Ok(ProjectId {
            value: value.to_string(),
            slash: owner.len(),
        })
    }

    /// `GitHub上の表記のままのowner`。
    pub fn owner(&self) -> &str {
        self.value.get(..self.slash).unwrap_or_default()
    }

    /// `GitHub上の表記のままのrepository`。
    pub fn repository(&self) -> &str {
        self.value.get(self.slash + 1..).unwrap_or_default()
    }

    /// 比較の正本となるASCII lowercase形式。
    ///
    /// ASCII lowercase化はbyte長を変えないため、slashの位置はそのまま持ち越せる。
    pub fn canonical(&self) -> CanonicalProjectId {
        CanonicalProjectId {
            value: self.value.to_ascii_lowercase(),
            slash: self.slash,
        }
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

/// `[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?`
fn is_valid_owner(owner: &str) -> bool {
    let bytes = owner.as_bytes();
    if bytes.is_empty() || bytes.len() > 39 {
        return false;
    }
    let alphanumeric = |b: u8| b.is_ascii_alphanumeric();
    if !alphanumeric(bytes[0]) || !alphanumeric(bytes[bytes.len() - 1]) {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'-')
}

/// `[A-Za-z0-9._-]{1,100}`
fn is_valid_repository(repository: &str) -> bool {
    let bytes = repository.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

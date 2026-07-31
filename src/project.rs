//! Project識別子。
//!
//! stringを直接workflowへ渡さず、validation済みの型を使う。案件の突き合わせには
//! ASCII lowercaseのcanonical形式を使い、表示にはGitHub上の表記を使う。

use crate::error::{Error, ErrorId, Result, fail};
use crate::hash::{SHORT_HEX_LENGTH, sha256_hex};
use crate::msg;

/// Sandbox名の固定接頭辞。
const SANDBOX_NAME_PREFIX: &str = "sbxm-";
/// Sandbox名の上限。
const SANDBOX_NAME_MAX_BYTES: usize = 63;

/// `<owner>/<repository>`。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectId {
    value: String,
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
        let (owner, repository) = match (parts.next(), parts.next(), parts.next()) {
            (Some(owner), Some(repository), None) => (owner, repository),
            _ => return Err(invalid()),
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
        })
    }

    /// GitHub上の表記のままのowner。
    pub fn owner(&self) -> &str {
        self.split().0
    }

    /// GitHub上の表記のままのrepository。
    pub fn repository(&self) -> &str {
        self.split().1
    }

    /// 比較の正本となるASCII lowercase形式。
    pub fn canonical(&self) -> CanonicalProjectId {
        CanonicalProjectId {
            value: self.value.to_ascii_lowercase(),
        }
    }

    fn split(&self) -> (&str, &str) {
        self.value
            .split_once('/')
            .expect("a parsed project ID has exactly one slash")
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

/// 比較に使うASCII lowercaseの`<owner>/<repository>`。
///
/// 案件の同一性はこの形式だけで判定する。表示には[`ProjectId`]の表記を使う。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalProjectId {
    value: String,
}

impl CanonicalProjectId {
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// lowercase化したrepository。host pathとSandbox内pathに使う。
    pub fn repository(&self) -> &str {
        self.value
            .split_once('/')
            .expect("a canonical project ID has exactly one slash")
            .1
    }
}

impl std::fmt::Display for CanonicalProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

/// canonical project IDから決定的に導出したSandbox名。
///
/// 同じcanonical project IDは常に同じ名前となり、異なるIDは通常hashで区別する。
/// hash prefixの理論上の衝突は、案件一覧を突き合わせる側がname collisionとして扱う。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SandboxName {
    value: String,
}

impl SandboxName {
    /// 1. `/`を`-`へ変え、`[a-z0-9-]`以外を`-`へ置換する
    /// 2. 連続する`-`を1個へ畳み、前後の`-`を除く
    /// 3. canonical project IDのSHA-256先頭12桁を求める
    /// 4. `sbxm-<slug>-<hash>`が63 byte以内になるようslugの末尾を切る
    pub fn derive(id: &CanonicalProjectId) -> SandboxName {
        let hash = sha256_hex(id.as_str().as_bytes());
        let hash = &hash[..SHORT_HEX_LENGTH];
        let budget = SANDBOX_NAME_MAX_BYTES - SANDBOX_NAME_PREFIX.len() - 1 - SHORT_HEX_LENGTH;

        let slug = slugify(id.as_str());
        let mut slug = slug.into_bytes();
        slug.truncate(budget);
        while slug.last() == Some(&b'-') {
            slug.pop();
        }
        let slug = String::from_utf8(slug).expect("the slug holds ASCII only");

        let value = if slug.is_empty() {
            format!("{SANDBOX_NAME_PREFIX}{hash}")
        } else {
            format!("{SANDBOX_NAME_PREFIX}{slug}-{hash}")
        };
        SandboxName { value }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for SandboxName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

/// Sandbox内のpath。
///
/// bare repositoryとmanaged worktreeは、Sandbox内の`agent` homeの下に、案件名から
/// 決定的に導出したpathで置く。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxLayout {
    repository: String,
}

impl SandboxLayout {
    pub fn new(id: &CanonicalProjectId) -> SandboxLayout {
        SandboxLayout {
            repository: id.repository().to_string(),
        }
    }

    /// `/home/agent/work/<repository-lower>`
    ///
    /// このdirectory自体はworktreeではない。
    pub fn bare_root(&self) -> String {
        format!("/home/agent/work/{}", self.repository)
    }

    /// `<bare-root>/.git`
    pub fn bare_git_dir(&self) -> String {
        format!("{}/.git", self.bare_root())
    }

    /// `<repository-lower>.tree-<index>`。metadataが持つmanaged worktreeの名前。
    pub fn worktree_name(&self, index: u32) -> String {
        format!("{}.tree-{index}", self.repository)
    }

    /// `<bare-root>/<repository-lower>.tree-<index>`
    pub fn worktree(&self, index: u32) -> String {
        format!("{}/{}", self.bare_root(), self.worktree_name(index))
    }

    /// 案件が持つmanaged worktreeの名前。
    pub fn worktree_names(&self, count: u32) -> Vec<String> {
        (0..count).map(|index| self.worktree_name(index)).collect()
    }
}

/// `[a-z0-9-]`だけの文字列へ落とし、連続する`-`を1個へ畳む。
fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let mapped = match byte {
            b'a'..=b'z' | b'0'..=b'9' => byte as char,
            _ => '-',
        };
        if mapped == '-' && out.ends_with('-') {
            continue;
        }
        out.push(mapped);
    }
    out.trim_matches('-').to_string()
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

#[cfg(test)]
#[path = "project_test.rs"]
mod project_test;

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
    /// 既にcanonical形式である文字列を検証する。
    ///
    /// metadataが持つ`canonical_id`の読み込みに使う。大文字を含む値は、表示用の表記が
    /// canonical形式の位置へ書かれた不整合として拒否する。
    pub fn parse(value: &str) -> Result<CanonicalProjectId> {
        let id = ProjectId::parse(value)?;
        let canonical = id.canonical();
        if canonical.as_str() != value {
            return fail(
                ErrorId::InvalidProjectId,
                msg!("error-invalid-project-id", value = value),
            );
        }
        Ok(canonical)
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// lowercase化したowner。host pathの1階層目に使う。
    pub fn owner(&self) -> &str {
        self.split().0
    }

    /// lowercase化したrepository。host pathとSandbox内pathに使う。
    pub fn repository(&self) -> &str {
        self.split().1
    }

    fn split(&self) -> (&str, &str) {
        self.value
            .split_once('/')
            .expect("a canonical project ID has exactly one slash")
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
mod tests {
    use super::*;

    #[test]
    fn project_ids_keep_the_given_casing() {
        let id = ProjectId::parse("Example-Org/Example.Repo").expect("valid");
        assert_eq!(id.to_string(), "Example-Org/Example.Repo");
    }

    #[test]
    fn project_ids_require_exactly_one_slash() {
        for value in ["owner", "owner/repo/extra", "/repo", "owner/", ""] {
            let error = ProjectId::parse(value).expect_err("{value} must be rejected");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::InvalidProjectId),
                "value {value} produced the wrong error"
            );
        }
    }

    #[test]
    fn owner_and_repository_character_rules_are_enforced() {
        assert!(ProjectId::parse("a/b").is_ok());
        assert!(ProjectId::parse("a-b-c/repo_name.v2").is_ok());
        assert!(ProjectId::parse(&format!("{}/repo", "a".repeat(39))).is_ok());
        assert!(ProjectId::parse(&format!("{}/repo", "a".repeat(40))).is_err());
        assert!(ProjectId::parse("-owner/repo").is_err());
        assert!(ProjectId::parse("owner-/repo").is_err());
        assert!(ProjectId::parse("own.er/repo").is_err());
        assert!(ProjectId::parse(&format!("owner/{}", "r".repeat(100))).is_ok());
        assert!(ProjectId::parse(&format!("owner/{}", "r".repeat(101))).is_err());
        assert!(ProjectId::parse("owner/re po").is_err());
    }

    #[test]
    fn dot_and_dot_dot_are_not_repository_names() {
        assert!(ProjectId::parse("owner/.").is_err());
        assert!(ProjectId::parse("owner/..").is_err());
    }

    #[test]
    fn the_reserved_repository_name_is_rejected_case_insensitively() {
        for value in ["owner/.sbxm", "owner/.SBXM", "owner/.Sbxm"] {
            let error = ProjectId::parse(value).expect_err("reserved names are rejected");
            assert_eq!(error.first_id(), Some(ErrorId::ReservedRepositoryName));
        }
    }

    #[test]
    fn the_display_form_keeps_the_casing_while_the_canonical_form_folds_it() {
        let id = ProjectId::parse("Example-Org/Example.Repo").expect("valid");
        assert_eq!(id.owner(), "Example-Org");
        assert_eq!(id.repository(), "Example.Repo");
        assert_eq!(id.canonical().to_string(), "example-org/example.repo");
        assert_eq!(id.canonical().owner(), "example-org");
        assert_eq!(id.canonical().repository(), "example.repo");
    }

    #[test]
    fn identifiers_that_differ_only_in_case_are_the_same_project() {
        let lower = ProjectId::parse("example-org/example-repo").unwrap();
        let mixed = ProjectId::parse("Example-Org/Example-Repo").unwrap();
        assert_ne!(lower, mixed, "the display form keeps what the user typed");
        assert_eq!(
            lower.canonical(),
            mixed.canonical(),
            "case never makes two different projects"
        );
        assert_eq!(
            SandboxName::derive(&lower.canonical()),
            SandboxName::derive(&mixed.canonical())
        );
    }

    #[test]
    fn a_canonical_identifier_refuses_anything_that_is_not_already_folded() {
        assert_eq!(
            CanonicalProjectId::parse("owner/repo").unwrap().to_string(),
            "owner/repo"
        );
        for value in ["Owner/repo", "owner/Repo", "owner", "owner/repo/extra"] {
            let error =
                CanonicalProjectId::parse(value).expect_err("{value} is not a canonical ID");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::InvalidProjectId),
                "value {value} produced the wrong error"
            );
        }
    }

    fn sandbox_name(value: &str) -> String {
        SandboxName::derive(&ProjectId::parse(value).unwrap().canonical())
            .as_str()
            .to_string()
    }

    #[test]
    fn the_sandbox_name_is_the_slug_followed_by_the_identifier_digest() {
        // hashはcanonical project IDのUTF-8に対するSHA-256の先頭12桁である。
        assert_eq!(
            sandbox_name("example-org/example-repo"),
            "sbxm-example-org-example-repo-99a40327a69b"
        );
        assert_eq!(sandbox_name("a/b"), "sbxm-a-b-c14cddc033f6");
    }

    #[test]
    fn characters_outside_the_slug_alphabet_collapse_into_single_separators() {
        assert_eq!(
            sandbox_name("Owner/repo_name.v2"),
            format!(
                "sbxm-owner-repo-name-v2-{}",
                &crate::hash::sha256_hex(b"owner/repo_name.v2")[..12]
            )
        );
        // 先頭と末尾の`-`は残さない。
        let name = sandbox_name("owner/...");
        assert!(name.starts_with("sbxm-owner-"), "{name}");
        assert!(!name.contains("--"), "{name}");
    }

    #[test]
    fn long_identifiers_stay_within_the_sandbox_name_limit() {
        let owner = "o".repeat(39);
        let repository = "r".repeat(100);
        let name = sandbox_name(&format!("{owner}/{repository}"));

        assert!(
            name.len() <= SANDBOX_NAME_MAX_BYTES,
            "{name} is {} bytes",
            name.len()
        );
        assert!(name.starts_with("sbxm-ooo"), "{name}");
        assert!(
            !name.contains("--"),
            "truncation must not leave a dangling separator: {name}"
        );
        // 切り詰めても案件ごとのhashは失われない。
        assert!(name.ends_with(&crate::hash::sha256_hex(
            format!("{owner}/{repository}").as_bytes()
        )[..12]));
    }

    #[test]
    fn different_projects_get_different_sandbox_names_even_when_the_slug_matches() {
        // slugは同じでも、canonical IDが異なればhashで区別される。
        let first = sandbox_name("owner/repo-name");
        let second = sandbox_name("owner/repo.name");
        assert_ne!(first, second);
        assert!(first.starts_with("sbxm-owner-repo-name-"), "{first}");
        assert!(second.starts_with("sbxm-owner-repo-name-"), "{second}");
    }
}

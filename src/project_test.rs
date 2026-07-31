use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

#[test]
fn project_ids_keep_the_given_casing() -> Checked {
    let id = ProjectId::parse("Example-Org/Example.Repo").required_because("valid")?;
    assert_eq!(id.to_string(), "Example-Org/Example.Repo");
    Ok(())
}

#[test]
fn project_ids_require_exactly_one_slash() -> Checked {
    for value in ["owner", "owner/repo/extra", "/repo", "owner/", ""] {
        let error = ProjectId::parse(value).refused_because("{value} must be rejected")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::InvalidProjectId),
            "value {value} produced the wrong error"
        );
    }
    Ok(())
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
fn the_reserved_repository_name_is_rejected_case_insensitively() -> Checked {
    for value in ["owner/.sbxm", "owner/.SBXM", "owner/.Sbxm"] {
        let error = ProjectId::parse(value).refused_because("reserved names are rejected")?;
        assert_eq!(error.first_id(), Some(ErrorId::ReservedRepositoryName));
    }
    Ok(())
}

#[test]
fn the_display_form_keeps_the_casing_while_the_canonical_form_folds_it() -> Checked {
    let id = ProjectId::parse("Example-Org/Example.Repo").required_because("valid")?;
    assert_eq!(id.owner(), "Example-Org");
    assert_eq!(id.repository(), "Example.Repo");
    assert_eq!(id.canonical().to_string(), "example-org/example.repo");
    assert_eq!(id.canonical().repository(), "example.repo");
    Ok(())
}

#[test]
fn identifiers_that_differ_only_in_case_are_the_same_project() -> Checked {
    let lower = ProjectId::parse("example-org/example-repo").required()?;
    let mixed = ProjectId::parse("Example-Org/Example-Repo").required()?;
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
    Ok(())
}

fn sandbox_name(value: &str) -> Checked<String> {
    Ok(
        SandboxName::derive(&ProjectId::parse(value).required()?.canonical())
            .as_str()
            .to_string(),
    )
}

#[test]
fn the_sandbox_name_is_the_slug_followed_by_the_identifier_digest() -> Checked {
    // hashはcanonical project IDのUTF-8に対するSHA-256の先頭12桁である。
    assert_eq!(
        sandbox_name("example-org/example-repo")?,
        "sbxm-example-org-example-repo-99a40327a69b"
    );
    assert_eq!(sandbox_name("a/b")?, "sbxm-a-b-c14cddc033f6");
    Ok(())
}

#[test]
fn characters_outside_the_slug_alphabet_collapse_into_single_separators() -> Checked {
    assert_eq!(
        sandbox_name("Owner/repo_name.v2")?,
        format!(
            "sbxm-owner-repo-name-v2-{}",
            &crate::hash::sha256_hex(b"owner/repo_name.v2")[..12]
        )
    );
    // 先頭と末尾の`-`は残さない。
    let name = sandbox_name("owner/...")?;
    assert!(name.starts_with("sbxm-owner-"), "{name}");
    assert!(!name.contains("--"), "{name}");
    Ok(())
}

#[test]
fn long_identifiers_stay_within_the_sandbox_name_limit() -> Checked {
    let owner = "o".repeat(39);
    let repository = "r".repeat(100);
    let name = sandbox_name(&format!("{owner}/{repository}"))?;

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
    assert!(
        name.ends_with(&crate::hash::sha256_hex(format!("{owner}/{repository}").as_bytes())[..12])
    );
    Ok(())
}

#[test]
fn different_projects_get_different_sandbox_names_even_when_the_slug_matches() -> Checked {
    // slugは同じでも、canonical IDが異なればhashで区別される。
    let first = sandbox_name("owner/repo-name")?;
    let second = sandbox_name("owner/repo.name")?;
    assert_ne!(first, second);
    assert!(first.starts_with("sbxm-owner-repo-name-"), "{first}");
    assert!(second.starts_with("sbxm-owner-repo-name-"), "{second}");
    Ok(())
}

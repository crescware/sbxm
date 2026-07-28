use super::*;

#[test]
fn ordinary_branch_names_are_accepted() {
    for value in ["main", "develop", "feature/login", "release-1.2", "v2"] {
        assert!(
            validate_branch_name(value).is_ok(),
            "{value} must be accepted"
        );
    }
    assert!(validate_branch_name(&"b".repeat(255)).is_ok());
}

#[test]
fn branch_names_that_could_be_misread_by_an_external_command_are_refused() {
    for value in [
        "",
        "-delete",
        "with\nnewline",
        "with\0nul",
        &"b".repeat(256),
    ] {
        let error = validate_branch_name(value).expect_err("{value:?} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::InvalidBranchName),
            "value {value:?} produced the wrong error"
        );
    }
}

#[test]
fn the_same_repository_normalizes_to_one_identifier_whichever_url_form_is_used() {
    for url in [
        "git@github.com:Example-Org/Example-Repo.git",
        "git@github.com:Example-Org/Example-Repo",
        "https://github.com/Example-Org/Example-Repo.git",
        "https://github.com/example-org/example-repo/",
        "ssh://git@github.com/Example-Org/Example-Repo.git",
        "ssh://git@github.com:22/example-org/example-repo.git",
        "  https://github.com/example-org/example-repo  ",
    ] {
        assert_eq!(
            canonical_id_of_remote(url).as_deref(),
            Some("example-org/example-repo"),
            "{url} must normalize to the canonical project ID"
        );
    }
}

#[test]
fn remotes_that_do_not_name_a_github_repository_are_not_normalized() {
    for url in [
        "git@gitlab.com:example-org/example-repo.git",
        "https://example.com/example-org/example-repo.git",
        "https://github.com/example-org",
        "https://github.com/example-org/nested/repo.git",
        "/srv/git/example-repo.git",
        "",
    ] {
        assert_eq!(
            canonical_id_of_remote(url),
            None,
            "{url} is not a GitHub repository this build can manage"
        );
    }
}

#[test]
fn remote_urls_are_built_from_the_display_names() {
    assert_eq!(
        ssh_remote_url("Example-Org", "Example-Repo"),
        "git@github.com:Example-Org/Example-Repo.git"
    );
    assert_eq!(
        https_remote_url("Example-Org", "Example-Repo"),
        "https://github.com/Example-Org/Example-Repo.git"
    );
    // 表記が違っても、正規化すれば同じ案件を指す。
    assert_eq!(
        canonical_id_of_remote(&ssh_remote_url("Example-Org", "Example-Repo")),
        canonical_id_of_remote(&https_remote_url("example-org", "example-repo"))
    );
}

#[test]
fn a_branch_name_becomes_the_remote_tracking_ref_of_origin() {
    assert_eq!(origin_ref("develop"), "refs/remotes/origin/develop");
    assert_eq!(
        origin_ref("feature/login"),
        "refs/remotes/origin/feature/login"
    );
}

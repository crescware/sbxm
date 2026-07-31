use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

fn parsed(value: &str) -> Checked<RepositoryIdentity> {
    RepositoryIdentity::parse_clone_url(value).required_because("the clone URL is accepted")
}

#[test]
fn the_ssh_clone_url_github_shows_is_accepted() -> Checked {
    let identity = parsed("git@github.com:Example-Org/Example-Repo.git")?;

    assert_eq!(identity.provider(), Provider::Github);
    assert_eq!(identity.owner(), "Example-Org");
    assert_eq!(identity.name(), "Example-Repo");
    assert_eq!(identity.canonical_id().as_str(), "example-org/example-repo");
    assert_eq!(identity.transport(), CloneTransport::Ssh);
    assert_eq!(
        identity.clone_url(),
        "git@github.com:Example-Org/Example-Repo.git"
    );
    assert_eq!(identity.display_id(), "Example-Org/Example-Repo");
    Ok(())
}

#[test]
fn the_https_clone_url_github_shows_is_accepted() -> Checked {
    let identity = parsed("https://github.com/Example-Org/Example-Repo.git")?;

    assert_eq!(identity.provider(), Provider::Github);
    assert_eq!(identity.owner(), "Example-Org");
    assert_eq!(identity.name(), "Example-Repo");
    assert_eq!(identity.canonical_id().as_str(), "example-org/example-repo");
    assert_eq!(identity.transport(), CloneTransport::Https);
    assert_eq!(
        identity.clone_url(),
        "https://github.com/Example-Org/Example-Repo.git"
    );
    Ok(())
}

#[test]
fn the_stored_clone_url_keeps_the_spelling_the_user_pasted() -> Checked {
    // hostだけを正規化し、ownerとrepositoryの表記はGitHub上のまま残す。
    let identity = parsed("git@GitHub.com:Example-Org/Example-Repo.git")?;
    assert_eq!(
        identity.clone_url(),
        "git@github.com:Example-Org/Example-Repo.git"
    );
    Ok(())
}

#[test]
fn a_bare_owner_slash_repository_is_not_a_clone_url() -> Checked {
    let error = RepositoryIdentity::parse_clone_url("owner/repository")
        .refused_because("the short form is not accepted")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidCloneUrl));
    Ok(())
}

#[test]
fn the_rejection_names_both_accepted_forms() -> Checked {
    let error =
        RepositoryIdentity::parse_clone_url("owner/repository").refused_because("not accepted")?;
    let described = error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .description
        .clone();
    let accepted = described
        .args
        .iter()
        .find(|(key, _)| *key == "accepted")
        .map(|(_, value)| value.clone())
        .required_because("the accepted forms are shown")?;
    assert!(accepted.contains(SSH_CLONE_URL_FORM), "{accepted}");
    assert!(accepted.contains(HTTPS_CLONE_URL_FORM), "{accepted}");
    Ok(())
}

#[test]
fn only_the_two_published_forms_are_accepted() -> Checked {
    for value in [
        // `.git`がない
        "git@github.com:owner/repository",
        "https://github.com/owner/repository",
        // 別のtransport表記
        "ssh://git@github.com/owner/repository.git",
        "ssh://github.com/owner/repository.git",
        "http://github.com/owner/repository.git",
        "git://github.com/owner/repository.git",
        // 別のhost
        "git@gitlab.com:owner/repository.git",
        "https://gitlab.com/owner/repository.git",
        "https://github.example.com/owner/repository.git",
        // 別のSSH user
        "hub@github.com:owner/repository.git",
        "github.com:owner/repository.git",
        // credential、port、query、fragment
        "https://user@github.com/owner/repository.git",
        "https://user:token@github.com/owner/repository.git",
        "https://github.com:443/owner/repository.git",
        "https://github.com/owner/repository.git?ref=main",
        "https://github.com/owner/repository.git#readme",
        // path要素数
        "https://github.com/owner/group/repository.git",
        "git@github.com:owner/group/repository.git",
        "https://github.com/repository.git",
        "https://github.com/owner/repository.git/",
        // 空のownerまたはrepository
        "git@github.com:/repository.git",
        "git@github.com:owner/.git",
        "https://github.com//repository.git",
        // 空文字
        "",
    ] {
        let error = RepositoryIdentity::parse_clone_url(value)
            .refused_because("{value} must not be accepted")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::InvalidCloneUrl),
            "value {value} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn owner_and_repository_still_obey_the_project_identifier_rules() -> Checked {
    let error = RepositoryIdentity::parse_clone_url("git@github.com:-owner/repository.git")
        .refused_because("the owner is not a project identifier")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidProjectId));

    let error = RepositoryIdentity::parse_clone_url("https://github.com/owner/.sbxm.git")
        .refused_because("the repository name is reserved")?;
    assert_eq!(error.first_id(), Some(ErrorId::ReservedRepositoryName));
    Ok(())
}

#[test]
fn the_same_repository_over_a_different_transport_is_a_different_target() -> Checked {
    let ssh = parsed("git@github.com:Example-Org/Example-Repo.git")?;
    let https = parsed("https://github.com/Example-Org/Example-Repo.git")?;
    assert!(!ssh.same_target(&https));
    assert_eq!(ssh.canonical_id(), https.canonical_id());
    Ok(())
}

#[test]
fn only_the_display_casing_may_differ_between_the_same_target() -> Checked {
    let stored = parsed("git@github.com:Example-Org/Example-Repo.git")?;
    let observed = parsed("git@github.com:example-org/example-repo.git")?;
    assert!(stored.same_target(&observed));
    assert_ne!(stored.clone_url(), observed.clone_url());
    Ok(())
}

#[test]
fn stored_fields_are_read_back_through_the_clone_url() -> Checked {
    let identity = RepositoryIdentity::from_parts(
        "github",
        "Example-Org",
        "Example-Repo",
        "example-org/example-repo",
        "ssh",
        "git@github.com:Example-Org/Example-Repo.git",
    )
    .required_because("the stored fields agree")?;
    assert_eq!(
        identity,
        parsed("git@github.com:Example-Org/Example-Repo.git")?
    );
    Ok(())
}

#[test]
fn stored_fields_that_disagree_with_the_clone_url_are_refused() -> Checked {
    let url = "git@github.com:Example-Org/Example-Repo.git";
    for (provider, owner, name, canonical, transport) in [
        // 未知のprovider、未知のtransport
        (
            "gitlab",
            "Example-Org",
            "Example-Repo",
            "example-org/example-repo",
            "ssh",
        ),
        (
            "github",
            "Example-Org",
            "Example-Repo",
            "example-org/example-repo",
            "git",
        ),
        // clone URLと食い違うfield
        (
            "github",
            "Other-Org",
            "Example-Repo",
            "example-org/example-repo",
            "ssh",
        ),
        (
            "github",
            "Example-Org",
            "Other-Repo",
            "example-org/example-repo",
            "ssh",
        ),
        (
            "github",
            "Example-Org",
            "Example-Repo",
            "other-org/example-repo",
            "ssh",
        ),
        (
            "github",
            "Example-Org",
            "Example-Repo",
            "example-org/example-repo",
            "https",
        ),
    ] {
        RepositoryIdentity::from_parts(provider, owner, name, canonical, transport, url)
            .refused_because("a disagreeing field set is refused")?;
    }

    RepositoryIdentity::from_parts(
        "github",
        "owner",
        "repository",
        "owner/repository",
        "ssh",
        "git@github.com:owner/repository",
    )
    .refused_because("a clone URL that is not one of the accepted forms is refused")?;
    Ok(())
}

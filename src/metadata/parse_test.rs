use super::*;
use crate::metadata::render;
use crate::testing::metadata::{OTHER_DIGEST, attached};
use crate::testing::value::DIGEST;

fn round_trip(metadata: &ProjectMetadata) -> ProjectMetadata {
    parse(&render(metadata), Path::new("/tmp/project.toml")).expect("the rendered form parses")
}

#[test]
fn metadata_written_before_worktrees_stopped_being_recorded_still_parses() {
    // 記録していた時期のfile。managed worktreeは本数から導けるため読む必要がなく、
    // 残っていても案件の目標構成は変わらない。
    let text = "\
version = 1
owner = \"Example-Org\"
repository = \"Example-Repo\"
canonical_id = \"example-org/example-repo\"

[provisioning]
mode = \"detached\"
start_ref = \"develop\"
requested_worktrees = 2
dockerfile_sha256 = \"1111111111111111111111111111111111111111111111111111111111111111\"

[[worktrees.managed]]
path = \"example-repo.tree-0\"
created_from = \"refs/remotes/origin/develop\"
";
    let parsed = parse(text, Path::new("/tmp/project.toml")).expect("the older form parses");
    assert_eq!(parsed.provisioning.requested_worktrees, 2);
    assert_eq!(parsed.provisioning.start_ref.as_deref(), Some("develop"));
}

#[test]
fn metadata_round_trips_through_the_rendered_form() {
    let metadata = attached("Example-Org", "Example-Repo");
    assert_eq!(round_trip(&metadata), metadata);

    let detached = ProjectMetadata {
        provisioning: Provisioning {
            mode: CreationMode::Detached,
            start_ref: Some("develop".to_string()),
            requested_worktrees: 3,
            dockerfile_sha256: DIGEST.to_string(),
        },
        rebuild: Some(RebuildIntent {
            target_dockerfile_sha256: OTHER_DIGEST.to_string(),
            previous_dockerfile_sha256: DIGEST.to_string(),
        }),
        ..attached("example-org", "example-repo")
    };
    assert_eq!(round_trip(&detached), detached);
}

#[test]
fn an_attached_project_may_wait_for_the_remote_default_branch() {
    let mut metadata = attached("example-org", "example-repo");
    metadata.provisioning.start_ref = None;
    assert_eq!(round_trip(&metadata), metadata);

    // detached modeは起点branchの明示を必須とする。
    let text = render(&metadata).replace("attached", "detached");
    let error = parse(&text, Path::new("/tmp/project.toml"))
        .expect_err("a detached project needs a start branch");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
}

#[test]
fn an_unknown_version_is_diagnosed_before_other_fields() {
    let text = "version = 2\n";
    let error = parse(text, Path::new("/tmp/project.toml")).expect_err("unknown versions fail");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnknownVersion));
}

#[test]
fn required_fields_are_named_when_they_are_missing() {
    let full = render(&attached("example-org", "example-repo"));
    for field in [
        "owner = ",
        "repository = ",
        "canonical_id = ",
        "mode = ",
        "start_ref = ",
        "requested_worktrees = ",
        "dockerfile_sha256 = ",
    ] {
        let text: String = full
            .lines()
            .filter(|line| !line.starts_with(field))
            .map(|line| format!("{line}\n"))
            .collect();
        let error = parse(&text, Path::new("/tmp/project.toml")).expect_err("{field} is required");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::MetadataMissingField),
            "{field} produced the wrong error"
        );
    }
}

#[test]
fn the_canonical_identifier_must_match_the_display_names() {
    let text = render(&attached("Example-Org", "Example-Repo"))
        .replace("example-org/example-repo", "other-org/other-repo");
    let error = parse(&text, Path::new("/tmp/project.toml"))
        .expect_err("the canonical ID must fold from owner and repository");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));

    let text = render(&attached("example-org", "example-repo")).replace(
        "canonical_id = \"example-org",
        "canonical_id = \"Example-Org",
    );
    let error = parse(&text, Path::new("/tmp/project.toml"))
        .expect_err("the canonical ID is always folded");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
}

#[test]
fn values_outside_the_documented_range_are_refused() {
    let base = render(&attached("example-org", "example-repo"));
    for (from, to) in [
        ("requested_worktrees = 1", "requested_worktrees = 0"),
        ("requested_worktrees = 1", "requested_worktrees = 33"),
        ("mode = \"attached\"", "mode = \"half-attached\""),
        ("dockerfile_sha256 = \"1111", "dockerfile_sha256 = \"NOTHEX"),
    ] {
        let text = base.replace(from, to);
        assert_ne!(text, base, "the replacement {from} did not apply");
        let error = parse(&text, Path::new("/tmp/project.toml")).expect_err("{to} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::MetadataInvalidValue),
            "{to} produced the wrong error"
        );
    }
}

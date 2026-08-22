use crate::diagnostics::ErrorId;
use crate::metadata::{
    CreationMode, InitialProvisioningIntent, ProjectMetadata, Provisioning, RebuildIntent,
};
use std::path::Path;

use std::fmt::Write as _;

/// 行を改行付きで連ねる。
fn joined_lines<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    lines.fold(String::new(), |mut out, line| {
        // Stringへの書き込みは失敗しない。
        let _ = writeln!(out, "{line}");
        out
    })
}

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::metadata::render;
use crate::testing::metadata::{OTHER_DIGEST, attached};
use crate::testing::value::DIGEST;

fn round_trip(metadata: &ProjectMetadata) -> Checked<ProjectMetadata> {
    parse(&render(metadata)?, Path::new("/tmp/project.yaml"))
        .required_because("the rendered form parses")
}

/// top-level sectionを、その配下ごと落とす。
///
/// 入れ子のkeyを1つ消すのと、sectionそのものが無いのは別の記録である。後者を作る。
fn without_section(text: &str, section: &str) -> String {
    let heading = format!("{section}:");
    let mut dropping = false;
    let mut kept = Vec::new();
    for line in text.lines() {
        if !line.starts_with(' ') && !line.trim().is_empty() {
            dropping = line.starts_with(&heading);
        }
        if !dropping {
            kept.push(line);
        }
    }
    let dropped = joined_lines(kept.into_iter());
    assert_ne!(dropped, text, "{section} was not part of the document");
    dropped
}

/// 1つのfieldを別の値へ置き換える。
fn replaced(text: &str, from: &str, to: &str) -> String {
    let changed = text.replace(from, to);
    assert_ne!(changed, text, "the replacement {from} did not apply");
    changed
}

/// `DIGEST`とも`OTHER_DIGEST`とも異なる、3つ目の世代を指す固定値。
const PREVIOUS_DIGEST: &str = "3333333333333333333333333333333333333333333333333333333333333333";

/// 診断が名指しした宣言fileのfield。
fn field_of(error: &crate::diagnostics::Error) -> Checked<String> {
    error
        .diagnostics()
        .iter()
        .flat_map(|diagnostic| &diagnostic.facts)
        .find_map(|fact| match fact {
            crate::design::Fact::OneLine { label, value }
                if label.id == "diagnostic-field-label" =>
            {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the diagnostic names the field")
}

/// 拒否されることを前提に、error IDを取り出す。
fn refusal(text: &str) -> Checked<Option<ErrorId>> {
    let error = parse(text, Path::new("/tmp/project.yaml"))
        .refused_because("the document is not usable")?;
    Ok(error.first_id())
}

#[test]
fn a_document_that_is_not_a_mapping_is_reported_as_syntax() -> Checked {
    // 読めない記録は、欠けているfieldの名前ではなく、記録そのものの問題として報告する。
    for text in [
        "- version: 1\n",
        "version: 1\n\tindented with a tab\n",
        "@\n",
    ] {
        assert_eq!(
            refusal(text)?,
            Some(ErrorId::MetadataInvalidSyntax),
            "{text:?} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_document_without_a_version_is_missing_a_field_rather_than_unreadable() -> Checked {
    // 空のfileはnullとして読める。keyを1つも持たないmappingと同じ扱いにする。
    assert_eq!(refusal("")?, Some(ErrorId::MetadataMissingField));

    let full = render(&attached("example-org", "example-repo")?)?;
    assert_eq!(
        refusal(&without_section(&full, "version"))?,
        Some(ErrorId::MetadataMissingField)
    );
    Ok(())
}

#[test]
fn a_section_that_is_absent_altogether_is_named_the_same_way_a_field_is() -> Checked {
    let full = render(&attached("example-org", "example-repo")?)?;
    for section in ["repository", "provisioning", "git_identity"] {
        assert_eq!(
            refusal(&without_section(&full, section))?,
            Some(ErrorId::MetadataMissingField),
            "{section} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_start_branch_that_git_would_not_accept_is_refused() -> Checked {
    let full = render(&attached("example-org", "example-repo")?)?;
    // 先頭の`-`は外部commandのoptionに読める。改行と空の値は記録として成立しない。
    for branch in ["-leading-dash", "", "main\\nmore"] {
        assert_eq!(
            refusal(&replaced(
                &full,
                "start_ref: main",
                &format!("start_ref: \"{branch}\"")
            ))?,
            Some(ErrorId::MetadataInvalidValue),
            "{branch} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_recorded_identity_must_be_usable_as_the_name_git_will_write() -> Checked {
    let full = render(&attached("example-org", "example-repo")?)?;
    for field in ["user_name", "user_email"] {
        let text = joined_lines(
            full.lines()
                .filter(|line| !line.trim_start().starts_with(&format!("{field}:"))),
        );
        assert_eq!(
            refusal(&text)?,
            Some(ErrorId::MetadataMissingField),
            "a document without git_identity.{field} produced the wrong error"
        );
    }

    // 空欄と改行は、そのままcommitの署名になってしまう値である。
    for (from, to) in [
        ("user_name: Example User", "user_name: \"   \""),
        ("user_name: Example User", "user_name: \"Example\\nUser\""),
        ("user_email: user@example.com", "user_email: \"\""),
        (
            "user_email: user@example.com",
            "user_email: \"user@example.com\\r\"",
        ),
    ] {
        assert_eq!(
            refusal(&replaced(&full, from, to))?,
            Some(ErrorId::MetadataInvalidValue),
            "{to} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_half_written_rebuild_record_is_refused_rather_than_resumed() -> Checked {
    // 3つのdigestをすべて別の値にする。1つを置き換えたとき、どのfieldが拒否を
    // 出したのかが値だけで決まる。
    let switching = ProjectMetadata {
        rebuild: Some(RebuildIntent {
            target_dockerfile_sha256: OTHER_DIGEST.to_string(),
            previous_dockerfile_sha256: PREVIOUS_DIGEST.to_string(),
        }),
        ..attached("example-org", "example-repo")?
    };
    let full = render(&switching)?;

    // 世代交代の途中で止まった記録は、片側だけを信じて続きを進めない。
    for field in ["target_dockerfile_sha256", "previous_dockerfile_sha256"] {
        let text = joined_lines(
            full.lines()
                .filter(|line| !line.trim_start().starts_with(&format!("{field}:"))),
        );
        assert_eq!(
            refusal(&text)?,
            Some(ErrorId::MetadataMissingField),
            "{field} produced the wrong error"
        );
    }

    for (from, field) in [
        (OTHER_DIGEST, "rebuild.target_dockerfile_sha256"),
        (PREVIOUS_DIGEST, "rebuild.previous_dockerfile_sha256"),
    ] {
        let error = parse(
            &replaced(&full, from, "not-a-digest"),
            Path::new("/tmp/project.yaml"),
        )
        .refused_because("a value that is not a digest names no generation")?;
        assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
        // どちらの世代の値が読めなかったのかを、診断がfield名で述べる。
        assert_eq!(field_of(&error)?, field, "{from} produced the wrong field");
    }
    Ok(())
}

#[test]
fn a_half_written_initial_provisioning_record_is_refused_rather_than_ignored() -> Checked {
    let pending = ProjectMetadata {
        initial_provisioning: Some(InitialProvisioningIntent {
            target_dockerfile_sha256: OTHER_DIGEST.to_string(),
        }),
        ..attached("example-org", "example-repo")?
    };
    let full = render(&pending)?;
    assert_eq!(round_trip(&pending)?, pending);

    let missing = full.replace("target_dockerfile_sha256:", "other:");
    assert_eq!(
        refusal(&missing)?,
        Some(ErrorId::MetadataMissingField),
        "a missing intent target is not treated as no intent"
    );

    let error = parse(
        &full.replace(OTHER_DIGEST, "not-a-digest"),
        Path::new("/tmp/project.yaml"),
    )
    .refused_because("an invalid intent target is not accepted")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
    assert_eq!(
        field_of(&error)?,
        "initial_provisioning.target_dockerfile_sha256"
    );
    Ok(())
}

#[test]
fn metadata_written_before_worktrees_stopped_being_recorded_still_parses() -> Checked {
    // 記録していた時期のfile。managed worktreeは本数から導けるため読む必要がなく、
    // 残っていても案件の目標構成は変わらない。
    let text = "\
version: 1
repository:
  provider: github
  owner: Example-Org
  name: Example-Repo
  canonical_id: example-org/example-repo
  clone_transport: ssh
  clone_url: git@github.com:Example-Org/Example-Repo.git

provisioning:
  mode: detached
  start_ref: develop
  requested_worktrees: 2
  dockerfile_sha256: \"1111111111111111111111111111111111111111111111111111111111111111\"

git_identity:
  user_name: Example User
  user_email: user@example.com

worktrees:
  managed:
    - path: example-repo.tree-0
      created_from: refs/remotes/origin/develop
";
    let parsed =
        parse(text, Path::new("/tmp/project.yaml")).required_because("the older form parses")?;
    assert_eq!(parsed.provisioning.requested_worktrees, 2);
    assert_eq!(parsed.provisioning.start_ref.as_deref(), Some("develop"));
    Ok(())
}

#[test]
fn metadata_round_trips_through_the_rendered_form() -> Checked {
    let metadata = attached("Example-Org", "Example-Repo")?;
    assert_eq!(round_trip(&metadata)?, metadata);

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
        ..attached("example-org", "example-repo")?
    };
    assert_eq!(round_trip(&detached)?, detached);
    Ok(())
}

#[test]
fn an_attached_project_may_wait_for_the_remote_default_branch() -> Checked {
    let mut metadata = attached("example-org", "example-repo")?;
    metadata.provisioning.start_ref = None;
    assert_eq!(round_trip(&metadata)?, metadata);

    // detached modeは起点branchの明示を必須とする。
    let text = render(&metadata)?.replace("attached", "detached");
    let error = parse(&text, Path::new("/tmp/project.yaml"))
        .refused_because("a detached project needs a start branch")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
    Ok(())
}

#[test]
fn an_unset_start_branch_is_told_apart_from_a_missing_record() -> Checked {
    let mut metadata = attached("example-org", "example-repo")?;
    metadata.provisioning.start_ref = None;

    // 未確定はnullとして書き、読み戻しても未確定のまま。
    let rendered = render(&metadata)?;
    assert!(
        rendered.contains("start_ref: null"),
        "an unset start branch is written as null: {rendered}"
    );

    // keyごと欠けている記録は、未確定ではなく欠落として報告する。
    let text = joined_lines(
        rendered
            .lines()
            .filter(|line| !line.trim_start().starts_with("start_ref:")),
    );
    let error = parse(&text, Path::new("/tmp/project.yaml"))
        .refused_because("a missing start_ref is not an unset start branch")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataMissingField));
    Ok(())
}

#[test]
fn a_start_branch_that_looks_like_yaml_syntax_survives_the_round_trip() -> Checked {
    // gitのbranch名としては妥当だが、YAMLとしては別の型や構造に読めてしまう値。
    for branch in [
        "no",
        "yes",
        "true",
        "null",
        "~",
        "123",
        "1.0",
        "#hash",
        "a: b",
        "*alias",
        "&anchor",
        "!tag",
        "@reserved",
        "  padded  ",
    ] {
        let mut metadata = attached("example-org", "example-repo")?;
        metadata.provisioning.start_ref = Some(branch.to_string());
        assert_eq!(
            round_trip(&metadata)?,
            metadata,
            "{branch:?} did not survive the round trip"
        );
    }
    Ok(())
}

#[test]
fn an_unknown_version_is_diagnosed_before_other_fields() -> Checked {
    let text = "version: 2\n";
    let error =
        parse(text, Path::new("/tmp/project.yaml")).refused_because("unknown versions fail")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnknownVersion));
    Ok(())
}

#[test]
fn required_fields_are_named_when_they_are_missing() -> Checked {
    let full = render(&attached("example-org", "example-repo")?)?;
    for field in [
        "provider:",
        "owner:",
        "name:",
        "canonical_id:",
        "clone_transport:",
        "clone_url:",
        "mode:",
        "start_ref:",
        "requested_worktrees:",
        "dockerfile_sha256:",
    ] {
        // 入れ子のkeyは字下げされているため、行頭の空白を除いてから見る。
        let text = joined_lines(
            full.lines()
                .filter(|line| !line.trim_start().starts_with(field)),
        );
        let error =
            parse(&text, Path::new("/tmp/project.yaml")).refused_because("{field} is required")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::MetadataMissingField),
            "{field} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn the_canonical_identifier_must_match_the_display_names() -> Checked {
    let text = render(&attached("Example-Org", "Example-Repo")?)?
        .replace("example-org/example-repo", "other-org/other-repo");
    let error = parse(&text, Path::new("/tmp/project.yaml"))
        .refused_because("the canonical ID must fold from owner and repository")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));

    let text = render(&attached("example-org", "example-repo")?)?
        .replace("canonical_id: example-org", "canonical_id: Example-Org");
    let error = parse(&text, Path::new("/tmp/project.yaml"))
        .refused_because("the canonical ID is always folded")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
    Ok(())
}

#[test]
fn values_outside_the_documented_range_are_refused() -> Checked {
    let base = render(&attached("example-org", "example-repo")?)?;
    for (from, to) in [
        ("requested_worktrees: 1", "requested_worktrees: 0"),
        ("requested_worktrees: 1", "requested_worktrees: 33"),
        ("mode: attached", "mode: half-attached"),
        (DIGEST, "not-a-digest"),
    ] {
        let text = base.replace(from, to);
        assert_ne!(text, base, "the replacement {from} did not apply");
        let error =
            parse(&text, Path::new("/tmp/project.yaml")).refused_because("{to} must be refused")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::MetadataInvalidValue),
            "{to} produced the wrong error"
        );
    }
    Ok(())
}

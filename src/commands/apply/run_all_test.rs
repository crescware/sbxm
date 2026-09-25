use crate::design::{Fact, SilentProgress};
use crate::diagnostics::ErrorId;
use crate::metadata::RebuildIntent;
use crate::project::{ProjectId, SandboxName};

use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::value::DIGEST;

use super::super::fake::*;
use super::*;

fn sandbox_of(project: &str) -> Checked<String> {
    Ok(SandboxName::derive(&ProjectId::parse(project).required()?.canonical()).to_string())
}

/// 指定した案件のSandboxを、指定した状態で並べる一覧。
fn listing_of(workspace_root: &Path, rows: &[(&str, &str)]) -> Checked<String> {
    let mut rendered = Vec::new();
    for (project, state) in rows {
        let name = sandbox_of(project)?;
        rendered.push(format!(
            r#"{{"name":"{name}","status":"{state}","workspaces":["{}"]}}"#,
            workspace_root.join(&name).display()
        ));
    }
    Ok(format!(r#"{{"sandboxes":[{}]}}"#, rendered.join(",")))
}

fn results(report: &AllReport) -> Vec<(&str, ProjectResult)> {
    report
        .outcomes
        .iter()
        .map(|outcome| (outcome.project.as_str(), outcome.result))
        .collect()
}

#[test]
fn every_registered_project_is_applied_by_the_state_of_its_own_sandbox() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = dir.path().join("declared.yaml");
    std::fs::write(&source, b"declared = true\n").required()?;
    let (_home, location, parent, config, workspace_root) = setup(vec![declaration(&source)?])?;
    write_metadata(&location, &parent, None)?;
    register_project(&location, &parent, "Example-Org/Stopped", None)?;
    register_project(&location, &parent, "Example-Org/Unbuilt", None)?;
    let host = FakeSbx::listing(&listing_of(
        &workspace_root,
        &[
            ("Example-Org/Example-Repo", "running"),
            ("Example-Org/Stopped", "stopped"),
        ],
    )?);

    let report = run_all(
        &location,
        &config,
        false,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .required_because("every project gets an outcome")?;

    assert_eq!(
        results(&report),
        vec![
            ("Example-Org/Example-Repo", ProjectResult::Applied),
            ("Example-Org/Stopped", ProjectResult::Stopped),
            ("Example-Org/Unbuilt", ProjectResult::NotCreated),
        ]
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    // 停止中のSandboxは起動しない。中でcommandを動かせば起動してしまう。
    let stopped = sandbox_of("Example-Org/Stopped")?;
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg != "ls") && args.contains(&stopped)),
        "nothing reaches the stopped sandbox: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_sandbox_that_already_holds_every_file_is_reported_as_unchanged() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = dir.path().join("declared.yaml");
    std::fs::write(&source, b"declared = true\n").required()?;
    let (_home, location, parent, config, workspace_root) = setup(vec![declaration(&source)?])?;
    write_metadata(&location, &parent, None)?;
    let destination = "/home/agent/.config/example/settings.yaml";
    let host = FakeSbx::listing(&listing_of(
        &workspace_root,
        &[("Example-Org/Example-Repo", "running")],
    )?)
    .holding(&[destination])
    .answering(
        &format!("sha256sum {destination}"),
        &format!(
            "{}  {destination}\n",
            crate::hash::sha256_hex(b"declared = true\n")
        ),
    );

    let report = run_all(
        &location,
        &config,
        false,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .required()?;

    assert_eq!(
        results(&report),
        vec![("Example-Org/Example-Repo", ProjectResult::Unchanged)]
    );
    assert!(!host.ran("exec -i"));
    Ok(())
}

#[test]
fn a_project_that_cannot_be_applied_does_not_stop_the_others() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = dir.path().join("declared.yaml");
    std::fs::write(&source, b"declared = true\n").required()?;
    let (_home, location, parent, config, workspace_root) = setup(vec![declaration(&source)?])?;
    // 世代交代の途中の案件は、単体の`apply`と同じく拒否する。
    register_project(
        &location,
        &parent,
        "Example-Org/Another",
        Some(RebuildIntent {
            target_dockerfile_sha256: DIGEST.into(),
            previous_dockerfile_sha256: DIGEST.into(),
        }),
    )?;
    write_metadata(&location, &parent, None)?;
    let host = FakeSbx::listing(&listing_of(
        &workspace_root,
        &[
            ("Example-Org/Another", "running"),
            ("Example-Org/Example-Repo", "running"),
        ],
    )?);

    let report = run_all(
        &location,
        &config,
        false,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .required_because("one refusal is an outcome, not the end of the run")?;

    assert_eq!(
        results(&report),
        vec![
            ("Example-Org/Another", ProjectResult::Failed),
            ("Example-Org/Example-Repo", ProjectResult::Applied),
        ]
    );
    let failure = report
        .failures
        .first()
        .required_because("the refusal is kept")?;
    assert_eq!(failure.id, ErrorId::RebuildIntentPending);
    // 同じ診断が案件ごとに並びうる。どの案件のものかを最初の事実で示す。
    assert_eq!(
        failure.facts.first(),
        Some(&Fact::project("Example-Org/Another"))
    );
    Ok(())
}

#[test]
fn nothing_registered_is_refused_rather_than_reported_as_done() -> Checked {
    let (_home, location, _parent, config, workspace_root) = setup(Vec::new())?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    let error = run_all(
        &location,
        &config,
        false,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("there is no project to place files in")?;
    assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
    Ok(())
}

use crate::command::{CommandSpec, HostEnvironment, TimeoutClass};
use crate::diagnostics::{ErrorId, Result};
use crate::hash::short_hex;
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, ProjectPaths};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::testing::outcome::{Checked, Refused, Required};

use super::{fake::*, *};
use crate::command::{CommandOutcome, OutputPolicy};
use crate::design::{Fact, SilentProgress};
use crate::testing::archive::image_archive_bytes;
use crate::testing::value::{DIGEST, IMAGE_ID};

#[test]
fn the_image_name_carries_the_sandbox_and_the_dockerfile_generation() -> Checked {
    assert_eq!(
        image_name(&sandbox()?, DIGEST),
        format!("{}-template:111111111111", sandbox()?)
    );
    Ok(())
}

#[test]
fn a_missing_image_is_built_into_an_empty_context_and_then_verified() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").required()?;
    // 1回目のinspectは不在、buildのあとは一致する。
    let host = FakeDocker::new(vec![Some(&matching_inspect()?), None]);

    let image = ensure(
        &host,
        &sandbox()?,
        &canonical()?,
        &dockerfile,
        DIGEST,
        &mut SilentProgress,
    )
    .required_because("build")?;
    assert!(image.built);
    assert_eq!(image.id, "sha256:image");

    let calls = host.calls();
    let build = calls
        .iter()
        .find(|args| args.first().is_some_and(|arg| arg == "build"))
        .required_because("the image is built")?;
    for (key, value) in declared_labels()? {
        assert!(
            build.contains(&format!("{key}={value}")),
            "the build declares {key}: {build:?}"
        );
    }
    assert!(build.contains(&"--tag".to_string()));
    assert!(build.contains(&image.name));
    assert!(build.contains(&paths::display(&dockerfile)));

    let context = build
        .last()
        .required_because("the context is the last argument")?;
    assert!(
        context.contains(BUILD_CONTEXT_PREFIX),
        "the build context is the ephemeral directory: {context}"
    );
    assert!(
        !std::path::Path::new(context).exists(),
        "the ephemeral context is removed once the build ends"
    );
    assert!(
        !context.starts_with(&paths::display(dir.path())),
        "the context never sits inside the project"
    );
    Ok(())
}

#[test]
fn an_image_that_declares_the_same_project_and_generation_is_reused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").required()?;
    let host = FakeDocker::new(vec![Some(&matching_inspect()?)]);

    let image = ensure(
        &host,
        &sandbox()?,
        &canonical()?,
        &dockerfile,
        DIGEST,
        &mut SilentProgress,
    )
    .required_because("reuse")?;
    assert!(!image.built);
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "build")),
        "a matching image is not rebuilt"
    );
    Ok(())
}

#[test]
fn a_context_that_cannot_be_removed_is_a_warning_not_a_failed_build() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").required()?;
    let host = FakeDocker::new(vec![Some(&matching_inspect()?), None]).losing_its_context();

    let image = ensure(
        &host,
        &sandbox()?,
        &canonical()?,
        &dockerfile,
        DIGEST,
        &mut SilentProgress,
    )
    .required_because("a context that is already gone does not undo the image")?;
    assert!(image.built);
    assert_eq!(image.warnings.len(), 1, "{:?}", image.warnings);
    assert_eq!(
        image.warnings[0].description.id,
        "warning-build-context-left-behind"
    );

    let path = image.warnings[0]
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-path-label" => {
                Some(value.as_str())
            }
            _ => None,
        })
        .required_because("the leftover directory is named")?;
    assert!(
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(BUILD_CONTEXT_PREFIX)),
        "{path} is not a build context"
    );
    Ok(())
}

#[test]
fn an_image_that_declares_something_else_is_a_collision_and_is_left_alone() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").required()?;
    let foreign = inspect_output(&[(LABEL_CANONICAL_ID, "other-org/other-repo")]);
    let host = FakeDocker::new(vec![Some(&matching_inspect()?), Some(&foreign)]);

    let error = ensure(
        &host,
        &sandbox()?,
        &canonical()?,
        &dockerfile,
        DIGEST,
        &mut SilentProgress,
    )
    .refused_because("the generation name is taken by something else")?;
    assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "build")),
        "an image sbxm did not build is never overwritten"
    );
    Ok(())
}

#[test]
fn an_engine_that_cannot_be_asked_is_not_read_as_an_absent_image() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").required()?;
    let host = FakeDocker::unreachable_engine();

    let error = ensure(
        &host,
        &sandbox()?,
        &canonical()?,
        &dockerfile,
        DIGEST,
        &mut SilentProgress,
    )
    .refused_because("an unobservable engine is not an image that is merely missing")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "build")),
        "nothing is built while the engine cannot be asked: {:?}",
        host.calls()
    );

    // 世代の判定も同じで、答えられないengineから世代を決めない。
    let host = FakeDocker::unreachable_engine();
    let error = generation_is_built(&host, &sandbox()?, &canonical()?, DIGEST)
        .refused_because("the generation of a build is never guessed")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}

#[test]
fn an_image_that_exists_but_cannot_be_inspected_stops_the_run() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").required()?;
    // 一覧には現れるが、inspectが答えない。
    let host = FakeDocker::new(vec![Some("not json")]);

    let error = ensure(
        &host,
        &sandbox()?,
        &canonical()?,
        &dockerfile,
        DIGEST,
        &mut SilentProgress,
    )
    .refused_because("an image that cannot be read is not rebuilt over")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    Ok(())
}

#[test]
fn a_build_that_produces_the_wrong_labels_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").required()?;
    let wrong = inspect_output(&[(LABEL_CANONICAL_ID, "example-org/example-repo")]);
    let host = FakeDocker::new(vec![Some(&wrong), None]);

    let error = ensure(
        &host,
        &sandbox()?,
        &canonical()?,
        &dockerfile,
        DIGEST,
        &mut SilentProgress,
    )
    .refused_because("a build is only done when the result proves it")?;
    assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
    Ok(())
}

#[test]
fn a_failed_build_removes_its_context_and_stops_the_workflow() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").required()?;
    let host = FakeDocker::new(vec![None]).failing_build();

    let error = ensure(
        &host,
        &sandbox()?,
        &canonical()?,
        &dockerfile,
        DIGEST,
        &mut SilentProgress,
    )
    .refused_because("a failed build is a failed step")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));

    let calls = host.calls();
    let build = calls
        .iter()
        .find(|args| args.first().is_some_and(|arg| arg == "build"))
        .required_because("the build was attempted")?;
    let context = build.last().required()?;
    assert!(
        !std::path::Path::new(context).exists(),
        "the ephemeral context is removed even when the build fails"
    );
    Ok(())
}

#[test]
fn the_build_forwards_its_progress() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").required()?;
    let host = FakeDocker::new(vec![Some(&matching_inspect()?), None]);
    ensure(
        &host,
        &sandbox()?,
        &canonical()?,
        &dockerfile,
        DIGEST,
        &mut SilentProgress,
    )
    .required_because("build")?;

    let calls = host.calls.borrow();
    for spec in calls.iter() {
        let expected = if spec.args.first().is_some_and(|arg| arg == "build") {
            (OutputPolicy::Passthrough, TimeoutClass::ImageBuild)
        } else {
            (OutputPolicy::Capture, TimeoutClass::LocalFilesystem)
        };
        assert_eq!((spec.output, spec.timeout), expected, "{:?}", spec.args);
    }
    Ok(())
}

/// `docker image save`が書いたarchiveを模して置く。
fn save_archive(host: &FakeDocker, image_name: &str, image_id: &str) -> Checked {
    let calls = host.calls.borrow();
    let save = calls
        .iter()
        .find(|spec| {
            spec.args.first().is_some_and(|arg| arg == "image")
                && spec.args.get(1).is_some_and(|arg| arg == "save")
        })
        .required_because("the image was saved")?;
    let output = save
        .args
        .iter()
        .skip_while(|arg| *arg != "--output")
        .nth(1)
        .required_because("the save names an output path")?;
    let owned = declared_labels()?;
    let labels: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    fs::write(output, image_archive_bytes(image_name, image_id, &labels))
        .required_because("write the archive")?;
    Ok(())
}

/// `image save`の呼び出しでarchiveを書くhost。
struct SavingDocker {
    image_name: String,
    image_id: String,
    inner: FakeDocker,
}

impl HostEnvironment for SavingDocker {
    fn command_exists(&self, program: &str) -> bool {
        self.inner.command_exists(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let outcome = self.inner.run(spec)?;
        if spec.args.first().is_some_and(|arg| arg == "image")
            && spec.args.get(1).is_some_and(|arg| arg == "save")
        {
            save_archive(&self.inner, &self.image_name, &self.image_id).map_err(|unmet| {
                crate::diagnostics::Error::single(
                    crate::diagnostics::Diagnostic::new(
                        ErrorId::ArchiveUnusable,
                        msg!("error-archive-unusable"),
                    )
                    .fact(Fact::cause(&unmet.to_string())),
                )
            })?;
        }
        Ok(outcome)
    }
}

fn project_paths(dir: &Path) -> Checked<ProjectPaths> {
    let base = crate::paths::ProjectParent::at(dir).required_because("valid parent directory")?;
    let paths = ProjectPaths::derive(&base, &canonical()?);
    fs::create_dir_all(paths.cache_dir()).required_because("create the cache directory")?;
    Ok(paths)
}

fn built_image() -> Checked<BuiltImage> {
    Ok(BuiltImage {
        name: image_name(&sandbox()?, DIGEST),
        id: IMAGE_ID.to_string(),
        labels: expected_labels(&canonical()?, DIGEST),
        built: true,
        warnings: Vec::new(),
    })
}

fn saving_host(image: &BuiltImage) -> SavingDocker {
    SavingDocker {
        image_name: image.name.clone(),
        image_id: image.id.clone(),
        inner: FakeDocker::new(Vec::new()),
    }
}

#[test]
fn the_archive_is_written_to_a_temporary_path_and_then_moved_into_place() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let image = built_image()?;
    let host = saving_host(&image);

    let archive = ensure_archive(&host, &paths, &image, DIGEST, &mut SilentProgress)
        .required_because("save")?;
    assert_eq!(archive, paths.template_archive(short_hex(DIGEST)));
    assert!(archive.is_file());
    assert!(
        !paths.template_archive_temp(short_hex(DIGEST)).exists(),
        "the temporary archive does not survive a successful save"
    );

    let calls = host.inner.calls();
    let save = calls.last().required_because("the save is the last call")?;
    assert_eq!(save[0], "image");
    assert_eq!(save[1], "save");
    assert!(save.contains(&image.name));
    assert!(save.contains(&paths::display(
        &paths.template_archive_temp(short_hex(DIGEST))
    )));
    Ok(())
}

#[test]
fn an_interrupted_temporary_archive_is_replaced_rather_than_reused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let image = built_image()?;
    let temporary = paths.template_archive_temp(short_hex(DIGEST));
    fs::write(&temporary, b"a partial archive from an interrupted run").required()?;

    ensure_archive(
        &saving_host(&image),
        &paths,
        &image,
        DIGEST,
        &mut SilentProgress,
    )
    .required_because("save")?;

    let archive = paths.template_archive(short_hex(DIGEST));
    assert!(archive.is_file());
    assert_ne!(
        fs::read(&archive).required()?,
        b"a partial archive from an interrupted run".to_vec(),
        "the interrupted cache is never promoted"
    );
    Ok(())
}

#[test]
fn an_archive_that_holds_another_image_leaves_the_official_one_untouched() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let image = built_image()?;
    let archive = paths.template_archive(short_hex(DIGEST));
    fs::write(&archive, b"the archive from an earlier run").required()?;

    let host = SavingDocker {
        image_name: "sbxm-other-template:222222222222".to_string(),
        image_id: image.id.clone(),
        inner: FakeDocker::new(Vec::new()),
    };
    let error = ensure_archive(&host, &paths, &image, DIGEST, &mut SilentProgress)
        .refused_because("an archive of another image is not promoted")?;
    assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    assert_eq!(
        fs::read(&archive).required()?,
        b"the archive from an earlier run".to_vec(),
        "the verified archive of an earlier generation stays as it is"
    );
    Ok(())
}

#[test]
fn the_ephemeral_context_is_private_and_empty() -> Checked {
    let context = ephemeral_context().required_because("a context is created")?;
    let mode = fs::metadata(context.path())
        .required()?
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, PRIVATE_DIR_MODE);
    assert_eq!(fs::read_dir(context.path()).required()?.count(), 0);
    assert!(
        context
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(BUILD_CONTEXT_PREFIX))
    );
    Ok(())
}

use super::*;
use crate::command::{CommandOutcome, OutputPolicy};
use crate::project::ProjectId;
use crate::testing::value::DIGEST;
use std::cell::RefCell;
use std::os::unix::process::ExitStatusExt;

struct FakeDocker {
    /// `docker image inspect`が返す出力。`None`はimageが存在しない状態。
    inspect: RefCell<Vec<Option<String>>>,
    calls: RefCell<Vec<CommandSpec>>,
    build_fails: bool,
    /// Docker Engineへ問い合わせられない状態。
    listing_fails: bool,
}

impl FakeDocker {
    fn new(inspect: Vec<Option<&str>>) -> FakeDocker {
        FakeDocker {
            inspect: RefCell::new(
                inspect
                    .into_iter()
                    .map(|value| value.map(|text| text.to_string()))
                    .collect(),
            ),
            calls: RefCell::new(Vec::new()),
            build_fails: false,
            listing_fails: false,
        }
    }

    fn failing_build(mut self) -> FakeDocker {
        self.build_fails = true;
        self
    }

    fn unreachable_engine() -> FakeDocker {
        FakeDocker {
            listing_fails: true,
            ..FakeDocker::new(Vec::new())
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls
            .borrow()
            .iter()
            .map(|spec| spec.args.clone())
            .collect()
    }
}

impl HostEnvironment for FakeDocker {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        let sub = |index: usize, name: &str| spec.args.get(index).is_some_and(|arg| arg == name);
        let building = sub(0, "build");
        let saving = sub(0, "image") && sub(1, "save");
        let listing = sub(0, "image") && sub(1, "ls");
        let (code, stdout) = if building {
            (i32::from(self.build_fails), String::new())
        } else if saving {
            (0, String::new())
        } else if listing {
            // 一覧は、次にinspectされるimageが存在するかだけを示す。
            if self.listing_fails {
                (1, String::new())
            } else {
                let present = self
                    .inspect
                    .borrow()
                    .last()
                    .is_some_and(|value| value.is_some());
                if !present {
                    // 不在の回はinspectまで進まないため、ここで1件を消費する。
                    self.inspect.borrow_mut().pop();
                }
                (0, if present { "0123456789ab\n" } else { "" }.to_string())
            }
        } else {
            match self.inspect.borrow_mut().pop() {
                Some(Some(output)) => (0, output),
                _ => (1, String::new()),
            }
        };
        Ok(CommandOutcome {
            program: spec.program.clone(),
            args: spec.args.clone(),
            working_dir: spec.working_dir.clone(),
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
            stderr_lossy: false,
        })
    }
}

fn canonical() -> CanonicalProjectId {
    ProjectId::parse("example-org/example-repo")
        .unwrap()
        .canonical()
}

fn sandbox() -> SandboxName {
    SandboxName::derive(&canonical())
}

fn inspect_output(labels: &[(&str, &str)]) -> String {
    let labels = labels
        .iter()
        .map(|(key, value)| format!("\"{key}\":\"{value}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"[{{"Id":"sha256:image","Config":{{"Labels":{{{labels}}}}}}}]"#)
}

fn declared_labels() -> Vec<(&'static str, String)> {
    vec![
        (LABEL_CANONICAL_ID, canonical().to_string()),
        (LABEL_DOCKERFILE_SHA256, DIGEST.to_string()),
        (LABEL_METADATA_VERSION, METADATA_VERSION.to_string()),
    ]
}

fn matching_inspect() -> String {
    let owned = declared_labels();
    let borrowed: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    inspect_output(&borrowed)
}

#[test]
fn the_image_name_carries_the_sandbox_and_the_dockerfile_generation() {
    assert_eq!(
        image_name(&sandbox(), DIGEST),
        format!("{}-template:111111111111", sandbox())
    );
}

#[test]
fn a_missing_image_is_built_into_an_empty_context_and_then_verified() {
    let dir = tempfile::tempdir().unwrap();
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    // 1回目のinspectは不在、buildのあとは一致する。
    let host = FakeDocker::new(vec![Some(&matching_inspect()), None]);

    let image = ensure(&host, &sandbox(), &canonical(), &dockerfile, DIGEST).expect("build");
    assert!(image.built);
    assert_eq!(image.id, "sha256:image");

    let calls = host.calls();
    let build = calls
        .iter()
        .find(|args| args.first().is_some_and(|arg| arg == "build"))
        .expect("the image is built");
    for (key, value) in declared_labels() {
        assert!(
            build.contains(&format!("{key}={value}")),
            "the build declares {key}: {build:?}"
        );
    }
    assert!(build.contains(&"--tag".to_string()));
    assert!(build.contains(&image.name));
    assert!(build.contains(&paths::display(&dockerfile)));

    let context = build.last().expect("the context is the last argument");
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
}

#[test]
fn an_image_that_declares_the_same_project_and_generation_is_reused() {
    let dir = tempfile::tempdir().unwrap();
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    let host = FakeDocker::new(vec![Some(&matching_inspect())]);

    let image = ensure(&host, &sandbox(), &canonical(), &dockerfile, DIGEST).expect("reuse");
    assert!(!image.built);
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "build")),
        "a matching image is not rebuilt"
    );
}

#[test]
fn an_image_that_declares_something_else_is_a_collision_and_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    let foreign = inspect_output(&[(LABEL_CANONICAL_ID, "other-org/other-repo")]);
    let host = FakeDocker::new(vec![Some(&matching_inspect()), Some(&foreign)]);

    let error = ensure(&host, &sandbox(), &canonical(), &dockerfile, DIGEST)
        .expect_err("the generation name is taken by something else");
    assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "build")),
        "an image sbxm did not build is never overwritten"
    );
}

#[test]
fn an_engine_that_cannot_be_asked_is_not_read_as_an_absent_image() {
    let dir = tempfile::tempdir().unwrap();
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    let host = FakeDocker::unreachable_engine();

    let error = ensure(&host, &sandbox(), &canonical(), &dockerfile, DIGEST)
        .expect_err("an unobservable engine is not an image that is merely missing");
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
    let error = generation_is_built(&host, &sandbox(), &canonical(), DIGEST)
        .expect_err("the generation of a build is never guessed");
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
}

#[test]
fn an_image_that_exists_but_cannot_be_inspected_stops_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    // 一覧には現れるが、inspectが答えない。
    let host = FakeDocker::new(vec![Some("not json")]);

    let error = ensure(&host, &sandbox(), &canonical(), &dockerfile, DIGEST)
        .expect_err("an image that cannot be read is not rebuilt over");
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
}

#[test]
fn a_build_that_produces_the_wrong_labels_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    let wrong = inspect_output(&[(LABEL_CANONICAL_ID, "example-org/example-repo")]);
    let host = FakeDocker::new(vec![Some(&wrong), None]);

    let error = ensure(&host, &sandbox(), &canonical(), &dockerfile, DIGEST)
        .expect_err("a build is only done when the result proves it");
    assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
}

#[test]
fn a_failed_build_removes_its_context_and_stops_the_workflow() {
    let dir = tempfile::tempdir().unwrap();
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    let host = FakeDocker::new(vec![None]).failing_build();

    let error = ensure(&host, &sandbox(), &canonical(), &dockerfile, DIGEST)
        .expect_err("a failed build is a failed step");
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));

    let calls = host.calls();
    let build = calls
        .iter()
        .find(|args| args.first().is_some_and(|arg| arg == "build"))
        .expect("the build was attempted");
    let context = build.last().unwrap();
    assert!(
        !std::path::Path::new(context).exists(),
        "the ephemeral context is removed even when the build fails"
    );
}

#[test]
fn the_build_forwards_its_progress() {
    let dir = tempfile::tempdir().unwrap();
    let dockerfile = dir.path().join("Dockerfile");
    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    let host = FakeDocker::new(vec![Some(&matching_inspect()), None]);
    ensure(&host, &sandbox(), &canonical(), &dockerfile, DIGEST).expect("build");

    let calls = host.calls.borrow();
    for spec in calls.iter() {
        let expected = if spec.args.first().is_some_and(|arg| arg == "build") {
            (OutputPolicy::Passthrough, TimeoutClass::ImageBuild)
        } else {
            (OutputPolicy::Capture, TimeoutClass::LocalFilesystem)
        };
        assert_eq!((spec.output, spec.timeout), expected, "{:?}", spec.args);
    }
}

/// `docker image save`が書いたarchiveを模して置く。
fn save_archive(host: &FakeDocker, image_name: &str, image_id: &str) {
    let calls = host.calls.borrow();
    let save = calls
        .iter()
        .find(|spec| {
            spec.args.first().is_some_and(|arg| arg == "image")
                && spec.args.get(1).is_some_and(|arg| arg == "save")
        })
        .expect("the image was saved");
    let output = save
        .args
        .iter()
        .skip_while(|arg| *arg != "--output")
        .nth(1)
        .expect("the save names an output path");
    // 実物と同じく、archiveはimage configをlabelごと持つ。
    let rendered = declared_labels()
        .iter()
        .map(|(key, value)| format!("\"{key}\":\"{value}\""))
        .collect::<Vec<_>>()
        .join(",");
    let config = format!(r#"{{"config":{{"Labels":{{{rendered}}}}}}}"#);
    let hex = image_id.strip_prefix("sha256:").unwrap_or(image_id);
    fs::write(
        output,
        crate::archive::tar_bytes(&[
            (&format!("blobs/sha256/{hex}"), config.as_bytes()),
            (
                "manifest.json",
                crate::archive::manifest_json(image_name, image_id).as_bytes(),
            ),
        ]),
    )
    .expect("write the archive");
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
            save_archive(&self.inner, &self.image_name, &self.image_id);
        }
        Ok(outcome)
    }
}

fn project_paths(dir: &Path) -> ProjectPaths {
    let base = crate::paths::AbsoluteBasePath::new(dir).expect("valid base path");
    let paths = ProjectPaths::derive(&base, &canonical());
    fs::create_dir_all(paths.cache_dir()).expect("create the cache directory");
    paths
}

fn built_image() -> BuiltImage {
    BuiltImage {
        name: image_name(&sandbox(), DIGEST),
        id: "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        labels: expected_labels(&canonical(), DIGEST),
        built: true,
        warnings: Vec::new(),
    }
}

fn saving_host(image: &BuiltImage) -> SavingDocker {
    SavingDocker {
        image_name: image.name.clone(),
        image_id: image.id.clone(),
        inner: FakeDocker::new(Vec::new()),
    }
}

#[test]
fn the_archive_is_written_to_a_temporary_path_and_then_moved_into_place() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let image = built_image();
    let host = saving_host(&image);

    let archive = ensure_archive(&host, &paths, &image, DIGEST).expect("save");
    assert_eq!(archive, paths.template_archive(short_hex(DIGEST)));
    assert!(archive.is_file());
    assert!(
        !paths.template_archive_temp(short_hex(DIGEST)).exists(),
        "the temporary archive does not survive a successful save"
    );

    let calls = host.inner.calls();
    let save = calls.last().expect("the save is the last call");
    assert_eq!(save[0], "image");
    assert_eq!(save[1], "save");
    assert!(save.contains(&image.name));
    assert!(save.contains(&paths::display(
        &paths.template_archive_temp(short_hex(DIGEST))
    )));
}

#[test]
fn an_interrupted_temporary_archive_is_replaced_rather_than_reused() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let image = built_image();
    let temporary = paths.template_archive_temp(short_hex(DIGEST));
    fs::write(&temporary, b"a partial archive from an interrupted run").unwrap();

    ensure_archive(&saving_host(&image), &paths, &image, DIGEST).expect("save");

    let archive = paths.template_archive(short_hex(DIGEST));
    assert!(archive.is_file());
    assert_ne!(
        fs::read(&archive).unwrap(),
        b"a partial archive from an interrupted run".to_vec(),
        "the interrupted cache is never promoted"
    );
}

#[test]
fn an_archive_that_holds_another_image_leaves_the_official_one_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths(dir.path());
    let image = built_image();
    let archive = paths.template_archive(short_hex(DIGEST));
    fs::write(&archive, b"the archive from an earlier run").unwrap();

    let host = SavingDocker {
        image_name: "sbxm-other-template:222222222222".to_string(),
        image_id: image.id.clone(),
        inner: FakeDocker::new(Vec::new()),
    };
    let error = ensure_archive(&host, &paths, &image, DIGEST)
        .expect_err("an archive of another image is not promoted");
    assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    assert_eq!(
        fs::read(&archive).unwrap(),
        b"the archive from an earlier run".to_vec(),
        "the verified archive of an earlier generation stays as it is"
    );
}

#[test]
fn the_ephemeral_context_is_private_and_empty() {
    let context = ephemeral_context().expect("a context is created");
    let mode = fs::metadata(context.path()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, PRIVATE_DIR_MODE);
    assert_eq!(fs::read_dir(context.path()).unwrap().count(), 0);
    assert!(
        context
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(BUILD_CONTEXT_PREFIX))
    );
}

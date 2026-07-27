//! 案件専用Templateのimage。
//!
//! Dockerfileの世代ごとにimageを持ち、labelで案件と世代を宣言する。buildは、
//! project fileもsecretも入らない空の一時directoryをbuild contextとして使う。

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::archive;

use crate::command::{CommandSpec, HostEnvironment, TimeoutClass};
use crate::compatibility::{ImageIdentity, parse_image_inspect};
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::hash::short_hex;
use crate::metadata::METADATA_VERSION;
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope, ProjectPaths};
use crate::project::{CanonicalProjectId, SandboxName};

/// imageが宣言するlabel。案件と世代の対応を、image自身が持つ。
pub const LABEL_CANONICAL_ID: &str = "io.crescware.sbxm.canonical-id";
pub const LABEL_DOCKERFILE_SHA256: &str = "io.crescware.sbxm.dockerfile-sha256";
pub const LABEL_METADATA_VERSION: &str = "io.crescware.sbxm.metadata-version";

/// 一時build contextのprefix。中断して残った場合も、由来が分かるようにする。
const BUILD_CONTEXT_PREFIX: &str = "sbxm-build-context-";

/// `<sandbox-name>-template:<dockerfile-sha256-first-12-hex>`
pub fn image_name(sandbox: &SandboxName, dockerfile_sha256: &str) -> String {
    format!("{}-template:{}", sandbox, short_hex(dockerfile_sha256))
}

/// 案件と世代が一致することを宣言するlabelの組。
pub fn expected_labels(
    canonical: &CanonicalProjectId,
    dockerfile_sha256: &str,
) -> Vec<(String, String)> {
    vec![
        (LABEL_CANONICAL_ID.to_string(), canonical.to_string()),
        (
            LABEL_DOCKERFILE_SHA256.to_string(),
            dockerfile_sha256.to_string(),
        ),
        (
            LABEL_METADATA_VERSION.to_string(),
            METADATA_VERSION.to_string(),
        ),
    ]
}

/// 使用できる状態のimage。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltImage {
    pub name: String,
    /// `sha256:<hex>`。archiveとTemplateの対応を判定する正本。
    pub id: String,
    /// この実行でbuildしたか。
    pub built: bool,
    /// 成果物としては成立したが、利用者へ伝える必要がある事実。
    pub warnings: Vec<Msg>,
}

/// 世代に対応するimageを用意する。
///
/// 既存imageは、全labelが一致した場合だけ再利用する。
pub fn ensure(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    canonical: &CanonicalProjectId,
    dockerfile: &Path,
    dockerfile_sha256: &str,
) -> Result<BuiltImage> {
    let name = image_name(sandbox, dockerfile_sha256);
    let labels = expected_labels(canonical, dockerfile_sha256);

    if let Some(identity) = inspect(host, &name)?
        && labels_match(&identity, &labels)
    {
        return Ok(BuiltImage {
            name,
            id: identity.id,
            built: false,
            warnings: Vec::new(),
        });
    }

    let warnings = build(host, &name, &labels, dockerfile)?;

    let identity = inspect(host, &name)?.ok_or_else(|| {
        Error::new(
            ErrorId::ImageUnusable,
            msg!(
                "error-image-unusable",
                image = name,
                detail = "the image is absent right after it was built"
            ),
        )
    })?;
    if !labels_match(&identity, &labels) {
        return Err(mismatched_labels(&name, &identity, &labels));
    }

    Ok(BuiltImage {
        name,
        id: identity.id,
        built: true,
        warnings,
    })
}

/// project file、config、secretを含まない空のbuild contextでbuildする。
fn build(
    host: &dyn HostEnvironment,
    name: &str,
    labels: &[(String, String)],
    dockerfile: &Path,
) -> Result<Vec<Msg>> {
    let context = ephemeral_context()?;

    let mut args: Vec<String> = Vec::new();
    for (key, value) in labels {
        args.push("--label".to_string());
        args.push(format!("{key}={value}"));
    }
    args.push("--tag".to_string());
    args.push(name.to_string());
    args.push("--file".to_string());
    args.push(paths::display(dockerfile));
    args.push(paths::display(context.path()));

    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    // buildの進捗はdockerが出したまま転送する。
    let spec = CommandSpec::passthrough("docker", &[&["build"], borrowed.as_slice()].concat())
        .timeout(TimeoutClass::ImageBuild);
    let result = host
        .run(&spec)
        .and_then(|outcome| outcome.require_success());

    // 成功・失敗にかかわらず、この実行が作った一時directoryを削除する。
    let leftover = paths::display(context.path());
    let mut warnings = Vec::new();
    if let Err(error) = context.close() {
        // buildが成功していれば、cleanup失敗だけで成果物を失敗扱いにしない。
        warnings.push(msg!(
            "warning-build-context-left-behind",
            path = leftover,
            detail = error
        ));
    }
    result?;
    Ok(warnings)
}

/// `docker build`へ渡す、空で私有な一時directory。
fn ephemeral_context() -> Result<tempfile::TempDir> {
    let context = tempfile::Builder::new()
        .prefix(BUILD_CONTEXT_PREFIX)
        .permissions(std::fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .tempdir()
        .map_err(|error| {
            Error::new(
                ErrorId::AtomicWriteFailed,
                msg!(
                    "error-atomic-write-failed",
                    path = BUILD_CONTEXT_PREFIX,
                    detail = error
                ),
            )
        })?;

    let path = context.path().to_path_buf();
    if paths::is_symlink(&path) {
        return Err(PathScope::ProjectPath.symlink_error(&path));
    }
    let resolved = fs::canonicalize(&path)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
    let entries = fs::read_dir(&resolved)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&resolved, &error.to_string()))?
        .count();
    if entries != 0 {
        return Err(Error::new(
            ErrorId::BuildContextNotEmpty,
            msg!(
                "error-build-context-not-empty",
                path = paths::display(&resolved),
                observed = entries
            ),
        ));
    }
    Ok(context)
}

/// 世代別のTemplate archiveを作り直す。
///
/// archive工程へ到達するたびに新しく生成する。既存archiveの再利用による性能最適化は
/// MVPの対象外であり、いま検証したimageと同じものであることを毎回確かめる。
/// 正式なarchiveは、一時archiveの生成と検証が終わるまで変更しない。
pub fn ensure_archive(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    image: &BuiltImage,
    dockerfile_sha256: &str,
) -> Result<PathBuf> {
    let generation = short_hex(dockerfile_sha256);
    let temporary = paths.template_archive_temp(generation);
    let target = paths.template_archive(generation);

    // project lockを保持しているため、残っている一時fileは中断した実行の跡である。
    if paths::regular_file_exists(&temporary, PathScope::ProjectPath)? {
        fs::remove_file(&temporary).map_err(|error| {
            Error::new(
                ErrorId::AtomicWriteFailed,
                msg!(
                    "error-atomic-write-failed",
                    path = paths::display(&temporary),
                    detail = error
                ),
            )
        })?;
    }

    let spec = CommandSpec::passthrough(
        "docker",
        &[
            "image",
            "save",
            &image.name,
            "--output",
            &paths::display(&temporary),
        ],
    )
    .timeout(TimeoutClass::ImageBuild);
    host.run(&spec)?.require_success()?;

    archive::verify_holds_image(&temporary, &image.name, &image.id)?;
    paths::atomic_rename_into_place(&temporary, &target)?;
    Ok(target)
}

/// 世代に対応するimageが既にあるか。
///
/// 初回構築の途中でDockerfileが変わった場合に、どちらの世代で完成させるかを決める。
pub fn generation_is_built(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    canonical: &CanonicalProjectId,
    dockerfile_sha256: &str,
) -> Result<bool> {
    let name = image_name(sandbox, dockerfile_sha256);
    let labels = expected_labels(canonical, dockerfile_sha256);
    Ok(inspect(host, &name)?.is_some_and(|identity| labels_match(&identity, &labels)))
}

/// imageの現在の同一性。存在しない場合は`None`。
///
/// `docker image inspect`は不在でも他の失敗でも非ゼロで終わるため、それだけで
/// 不在と判定しない。まず一覧で存在を確かめ、observeできない状態はerrorとして返す。
fn inspect(host: &dyn HostEnvironment, name: &str) -> Result<Option<ImageIdentity>> {
    if !exists(host, name)? {
        return Ok(None);
    }
    let spec = CommandSpec::capture("docker", &["image", "inspect", name])
        .timeout(TimeoutClass::LocalFilesystem);
    let outcome = host.run(&spec)?.require_success()?;
    parse_image_inspect(&outcome.stdout_text()).map(Some)
}

/// 名前が一致するimageが存在するか。
///
/// 一覧の失敗は不在へ丸めず、そのまま呼び出し側の失敗にする。
fn exists(host: &dyn HostEnvironment, name: &str) -> Result<bool> {
    let spec = CommandSpec::capture("docker", &["image", "ls", "--quiet", name])
        .timeout(TimeoutClass::LocalFilesystem);
    let outcome = host.run(&spec)?.require_success()?;
    Ok(!outcome.stdout_text().trim().is_empty())
}

fn labels_match(identity: &ImageIdentity, expected: &[(String, String)]) -> bool {
    expected
        .iter()
        .all(|(key, value)| identity.labels.get(key) == Some(value))
}

fn mismatched_labels(name: &str, identity: &ImageIdentity, expected: &[(String, String)]) -> Error {
    let observed = expected
        .iter()
        .map(|(key, value)| {
            let observed = identity
                .labels
                .get(key)
                .map(String::as_str)
                .unwrap_or("<absent>");
            format!("{key}: expected {value}, observed {observed}")
        })
        .collect::<Vec<_>>()
        .join("; ");
    Error::single(Diagnostic::new(
        ErrorId::ImageUnusable,
        msg!("error-image-unusable", image = name, detail = observed),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutcome, OutputPolicy};
    use crate::project::ProjectId;
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
            let sub =
                |index: usize, name: &str| spec.args.get(index).is_some_and(|arg| arg == name);
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

    const DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";

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
    fn an_image_that_declares_something_else_is_rebuilt_rather_than_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let dockerfile = dir.path().join("Dockerfile");
        fs::write(&dockerfile, "FROM scratch\n").unwrap();
        let foreign = inspect_output(&[(LABEL_CANONICAL_ID, "other-org/other-repo")]);
        let host = FakeDocker::new(vec![Some(&matching_inspect()), Some(&foreign)]);

        let image = ensure(&host, &sandbox(), &canonical(), &dockerfile, DIGEST).expect("rebuild");
        assert!(image.built, "an image with foreign labels is not reused");
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
        fs::write(
            output,
            crate::archive::tar_bytes(&[(
                "manifest.json",
                crate::archive::manifest_json(image_name, image_id).as_bytes(),
            )]),
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
            id: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
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
}

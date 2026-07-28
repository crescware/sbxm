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
    /// `docker image inspect`が示した`Id`。
    ///
    /// image storeとattestationの有無で、config、manifest、image indexの
    /// どれを指すかが変わる。archiveとの対応の判定には使わない。
    pub id: String,
    /// このimageが宣言しているlabel。archiveとの対応はこれで判定する。
    pub labels: Vec<(String, String)>,
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

    if let Some(identity) = inspect(host, &name)? {
        if !labels_match(&identity, &labels) {
            // 世代名が同じでも中身は別物である。この名前の既存成果物を作り直さない。
            return Err(collision(&name, &identity, &labels));
        }
        return Ok(BuiltImage {
            name,
            id: identity.id,
            labels,
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
        labels,
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
    crate::progress::step(&msg!("progress-building-image"));
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

    crate::progress::step(&msg!("progress-saving-archive"));
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

    archive::verify_holds_image(&temporary, &image.name, &image.labels)?;
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
pub fn inspect(host: &dyn HostEnvironment, name: &str) -> Result<Option<ImageIdentity>> {
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
    Error::single(Diagnostic::new(
        ErrorId::ImageUnusable,
        msg!(
            "error-image-unusable",
            image = name,
            detail = compare_labels(identity, expected)
        ),
    ))
}

/// 同じ世代名を持つ、別の案件または別の世代のimage。
///
/// 名前だけで同一とみなして上書きすると、利用者の成果物を失う。
fn collision(name: &str, identity: &ImageIdentity, expected: &[(String, String)]) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ImageUnusable,
            msg!(
                "error-image-collision",
                image = name,
                detail = compare_labels(identity, expected)
            ),
        )
        .remediation(msg!("remediation-image-collision", image = name)),
    )
}

/// 期待するlabelと観測したlabelの並び。翻訳しない技術表記。
fn compare_labels(identity: &ImageIdentity, expected: &[(String, String)]) -> String {
    expected
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
        .join("; ")
}

#[cfg(test)]
#[path = "image_test.rs"]
mod image_test;

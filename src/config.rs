//! Global config `~/.sbxm/config.yaml`。
//!
//! configはtoken、secret、runtime状態を保存しない。不正なconfigを自動修復せず、
//! `init`も上書きしない。

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Diagnostic, DocumentVersion, Error, ErrorId, Result, fail};
use crate::i18n::Locale;
use crate::msg;
use crate::paths::{
    self, AbsoluteBasePath, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope, atomic_create,
    permission_too_open,
};
use crate::ui::Warning;

/// このbuildが読み書きするconfigのversion。
pub const CONFIG_VERSION: u32 = 1;

/// configのversionの読み方。
const DOCUMENT: DocumentVersion = DocumentVersion {
    supported: CONFIG_VERSION,
    unknown: ErrorId::ConfigUnknownVersion,
    unknown_message: "error-config-unknown-version",
};

/// version 1で意味を持つtop-level key。
const KNOWN_TOP_LEVEL_KEYS: &[&str] = &["version", "language", "base_path", "git", "files"];

/// `~/.sbxm`配下の固定path。
///
/// home directoryを明示的に受け取り、processのenvironmentに依存しない。
#[derive(Debug, Clone)]
pub struct ConfigLocation {
    home: PathBuf,
}

impl ConfigLocation {
    #[cfg(test)]
    pub fn from_home(home: PathBuf) -> ConfigLocation {
        ConfigLocation { home }
    }

    /// 現在の利用者のhome directoryから構築する。
    pub fn discover() -> Result<ConfigLocation> {
        let home = dirs::home_dir().ok_or_else(|| {
            Error::new(
                ErrorId::ConfigUnreadable,
                msg!(
                    "error-config-unreadable",
                    path = "~",
                    detail = "the home directory could not be determined"
                ),
            )
        })?;
        Ok(ConfigLocation { home })
    }

    /// `~/.sbxm`
    pub fn dir(&self) -> PathBuf {
        self.home.join(".sbxm")
    }

    /// `~/.sbxm/config.yaml`
    pub fn config_file(&self) -> PathBuf {
        self.dir().join("config.yaml")
    }

    /// `~/.sbxm/init.lock`
    pub fn init_lock(&self) -> PathBuf {
        self.dir().join("init.lock")
    }
}

/// Sandbox内の`agent` homeへ配置するhost file。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFileSource(PathBuf);

impl HostFileSource {
    pub fn new(value: &str) -> std::result::Result<HostFileSource, &'static str> {
        let path = PathBuf::from(value);
        if value.is_empty() {
            return Err("the source is empty");
        }
        if !path.is_absolute() {
            return Err("the source is not an absolute path");
        }
        Ok(HostFileSource(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// Sandbox内の`agent` homeからの相対path。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxHomeRelativePath(PathBuf);

impl SandboxHomeRelativePath {
    pub fn new(value: &str) -> std::result::Result<SandboxHomeRelativePath, &'static str> {
        let path = PathBuf::from(value);
        if value.is_empty() {
            return Err("the destination is empty");
        }
        if path.is_absolute() {
            return Err("the destination is an absolute path");
        }
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err("the destination contains a parent directory component");
        }
        Ok(SandboxHomeRelativePath(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// Sandbox内で使用するGit identity。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitIdentity {
    pub user_name: String,
    pub user_email: String,
}

/// host上の通常fileをSandbox内へ配置する宣言。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDeclaration {
    pub source: HostFileSource,
    pub destination: SandboxHomeRelativePath,
}

/// validation済みのglobal config。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalConfig {
    pub language: Locale,
    pub base_path: AbsoluteBasePath,
    pub git: GitIdentity,
    pub files: Vec<FileDeclaration>,
}

/// config loadの結果。
#[derive(Debug)]
pub enum ConfigState {
    /// configが存在しない。`init`は新規作成へ進み、他のcommandは`init`を案内する。
    Missing,
    /// 有効なconfig。version 1では未知のtop-level keyをwarningとして返す。
    Valid {
        config: Box<GlobalConfig>,
        warnings: Vec<Warning>,
    },
}

/// YAMLの生表現。structへ変換する前にtop-level keyを検査する。
#[derive(Debug, Deserialize, Serialize)]
struct RawConfig {
    version: Option<i64>,
    language: Option<String>,
    base_path: Option<String>,
    git: Option<RawGit>,
    #[serde(default)]
    files: Vec<RawFile>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawGit {
    user_name: Option<String>,
    user_email: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawFile {
    source: Option<String>,
    destination: Option<String>,
}

/// configをread-onlyで読み、存在と妥当性を判定する。
///
/// 構文不正、未知version、必須値欠落、permission過剰、symlink、relative base pathは
/// pathと原因を示すerrorとし、自動修復しない。
pub fn load(location: &ConfigLocation) -> Result<ConfigState> {
    let path = location.config_file();

    if paths::is_symlink(&path) {
        return Err(PathScope::ConfigFile.symlink_error(&path));
    }

    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ConfigState::Missing);
        }
        Err(error) => {
            return fail(
                ErrorId::ConfigUnreadable,
                msg!(
                    "error-config-unreadable",
                    path = paths::display(&path),
                    detail = error
                ),
            );
        }
    };

    if !metadata.is_file() {
        return fail(
            ErrorId::ConfigUnreadable,
            msg!(
                "error-config-unreadable",
                path = paths::display(&path),
                detail = "the configuration path is not a regular file"
            ),
        );
    }

    let mode = metadata.permissions().mode();
    if permission_too_open(mode) {
        return Err(PathScope::ConfigFile.permission_error(&path, mode, PRIVATE_FILE_MODE));
    }

    let text = fs::read_to_string(&path).map_err(|error| {
        Error::new(
            ErrorId::ConfigUnreadable,
            msg!(
                "error-config-unreadable",
                path = paths::display(&path),
                detail = error
            ),
        )
    })?;

    parse(&text, &path)
}

/// configのtextを検証する。filesystemには触れない部分の判定をまとめる。
fn parse(text: &str, path: &Path) -> Result<ConfigState> {
    let syntax_error = |error: yaml_serde::Error| {
        Error::new(
            ErrorId::ConfigInvalidSyntax,
            msg!(
                "error-config-invalid-syntax",
                path = paths::display(path),
                detail = error
            ),
        )
    };

    let document: yaml_serde::Value = yaml_serde::from_str(text).map_err(syntax_error)?;
    // 空のdocumentもcommentだけのdocumentもnullとして読める。keyを1つも持たない
    // mappingと同じ扱いにし、欠落したfieldをsyntax errorではなく名前で報告する。
    let document = if document.is_null() {
        yaml_serde::Value::Mapping(yaml_serde::Mapping::new())
    } else {
        document
    };

    let mut warnings = Vec::new();
    if let Some(mapping) = document.as_mapping() {
        for key in mapping.keys() {
            // YAMLのkeyは文字列とは限らない。既知keyはすべて文字列なので、
            // 文字列でないkeyはその表記のまま未知として報告する。
            let name = key
                .as_str()
                .map_or_else(|| format!("{key:?}"), str::to_string);
            if !KNOWN_TOP_LEVEL_KEYS.contains(&name.as_str()) {
                warnings.push(Warning::text(msg!(
                    "warning-config-unknown-key",
                    path = paths::display(path),
                    key = name
                )));
            }
        }
    }

    let raw: RawConfig = yaml_serde::from_value(document).map_err(syntax_error)?;

    let missing_field = |field: &'static str| {
        Error::new(
            ErrorId::ConfigMissingField,
            msg!(
                "error-config-missing-field",
                path = paths::display(path),
                field = field
            ),
        )
    };
    let invalid_value = |field: &'static str, detail: String| {
        Error::single(
            Diagnostic::new(
                ErrorId::ConfigInvalidValue,
                msg!(
                    "error-config-invalid-value",
                    path = paths::display(path),
                    field = field,
                    detail = detail
                ),
            )
            .remediation(msg!("remediation-fix-config", path = paths::display(path))),
        )
    };

    DOCUMENT.require(raw.version, &paths::display(path), || {
        missing_field("version")
    })?;

    let language_value = raw.language.ok_or_else(|| missing_field("language"))?;
    let language = Locale::parse_exact(&language_value).ok_or_else(|| {
        invalid_value(
            "language",
            format!(
                "{language_value} is not one of {}",
                supported_language_list()
            ),
        )
    })?;

    let base_path_value = raw.base_path.ok_or_else(|| missing_field("base_path"))?;
    let base_path = AbsoluteBasePath::new(Path::new(&base_path_value))?;

    let git = raw.git.ok_or_else(|| missing_field("git"))?;
    let user_name = git
        .user_name
        .ok_or_else(|| missing_field("git.user_name"))?;
    let user_email = git
        .user_email
        .ok_or_else(|| missing_field("git.user_email"))?;
    if let Err(detail) = validate_git_identity_value(&user_name) {
        return Err(invalid_value("git.user_name", detail.to_string()));
    }
    if let Err(detail) = validate_git_identity_value(&user_email) {
        return Err(invalid_value("git.user_email", detail.to_string()));
    }

    let mut files = Vec::with_capacity(raw.files.len());
    for (index, entry) in raw.files.into_iter().enumerate() {
        let source_value = entry.source.ok_or_else(|| missing_field("files.source"))?;
        let destination_value = entry
            .destination
            .ok_or_else(|| missing_field("files.destination"))?;
        let source = HostFileSource::new(&source_value).map_err(|detail| {
            Error::new(
                ErrorId::FileDeclarationInvalidSource,
                msg!(
                    "error-file-declaration-invalid-source",
                    index = index,
                    source = source_value,
                    detail = detail
                ),
            )
        })?;
        let destination = SandboxHomeRelativePath::new(&destination_value).map_err(|detail| {
            Error::new(
                ErrorId::FileDeclarationInvalidDestination,
                msg!(
                    "error-file-declaration-invalid-destination",
                    index = index,
                    destination = destination_value,
                    detail = detail
                ),
            )
        })?;
        files.push(FileDeclaration {
            source,
            destination,
        });
    }

    Ok(ConfigState::Valid {
        config: Box::new(GlobalConfig {
            language,
            base_path,
            git: GitIdentity {
                user_name,
                user_email,
            },
            files,
        }),
        warnings,
    })
}

fn supported_language_list() -> String {
    Locale::ALL
        .iter()
        .map(|locale| locale.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Git identityの値として使えるか。
pub fn validate_git_identity_value(value: &str) -> std::result::Result<(), &'static str> {
    if value.trim().is_empty() {
        return Err("detail-value-empty");
    }
    if value.contains('\n') || value.contains('\r') {
        return Err("detail-value-has-newline");
    }
    Ok(())
}

/// configをYAMLへ描画する。
///
/// 引用符の付け方は`yaml_serde`の判断に委ね、sbxmは介入しない。介入するなら危険な値の
/// listを持つか、YAMLを手で組み立てるかのどちらかになる。前者は維持できず、後者はこの
/// 関数がserializeへ移った理由そのものを捨てる。
///
/// 帰結として、出力はYAML 1.2として読まれることを前提とする。`no`や`yes`は引用されず、
/// YAML 1.1の実装から読めばbooleanになる。sbxmは自分が書いたfileを同じcrateで読むため、
/// 往復は一致する。
pub fn render(config: &GlobalConfig) -> String {
    let raw = RawConfig {
        version: Some(i64::from(CONFIG_VERSION)),
        language: Some(config.language.as_str().to_string()),
        base_path: Some(paths::display(config.base_path.as_path())),
        git: Some(RawGit {
            user_name: Some(config.git.user_name.clone()),
            user_email: Some(config.git.user_email.clone()),
        }),
        files: config
            .files
            .iter()
            .map(|declaration| RawFile {
                source: Some(paths::display(declaration.source.as_path())),
                destination: Some(paths::display(declaration.destination.as_path())),
            })
            .collect(),
    };
    // RawConfigは文字列と整数とVecだけで構成され、YAMLで表現できない値を持たない。
    yaml_serde::to_string(&raw).expect("a configuration is representable as YAML")
}

/// `~/.sbxm`を`0700`で検証または作成する。
pub fn ensure_config_dir(location: &ConfigLocation) -> Result<PathBuf> {
    let dir = location.dir();
    paths::ensure_private_dir(&dir, PRIVATE_DIR_MODE, PathScope::ConfigDir)?;
    Ok(dir)
}

/// configを新規作成する。既存configは上書きしない。
pub fn create(location: &ConfigLocation, config: &GlobalConfig) -> Result<PathBuf> {
    let path = location.config_file();
    atomic_create(&path, &render(config), PRIVATE_FILE_MODE)?;
    Ok(path)
}

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;

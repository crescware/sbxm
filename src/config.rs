//! Global config `~/.sbxm/config.yaml`。
//!
//! configは利用者設定だけを持つ。登録案件の索引は`registry.yaml`が持ち、責務を混ぜない。
//! configはtoken、secret、runtime状態を保存しない。
//!
//! fileが存在しないこと、および既知のoptional fieldが無いことは正常であり、default設定
//! として扱う。ただし、存在するconfigが構文不正、未知version、permission不正、symlink、
//! read失敗である場合はdefaultへfallbackせず拒否する。不正なconfigを自動修復しない。

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Diagnostic, DocumentVersion, Error, ErrorId, Result, fail};
use crate::i18n::Locale;
use crate::msg;
use crate::paths::{
    self, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope, atomic_create, permission_too_open,
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
const KNOWN_TOP_LEVEL_KEYS: &[&str] = &["version", "language", "files"];

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

    /// `~/.sbxm/registry.yaml`
    pub fn registry_file(&self) -> PathBuf {
        self.dir().join("registry.yaml")
    }

    /// `~/.sbxm/registry.lock`
    pub fn registry_lock(&self) -> PathBuf {
        self.dir().join("registry.lock")
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

/// host上の通常fileをSandbox内へ配置する宣言。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDeclaration {
    pub source: HostFileSource,
    pub destination: SandboxHomeRelativePath,
}

/// validation済みのglobal config。
///
/// 欠落したoptional fieldは未保存として扱う。表示言語が未保存であることと、
/// 特定の言語が保存されていることを同じ値にしない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalConfig {
    pub language: Option<Locale>,
    pub files: Vec<FileDeclaration>,
}

/// config loadの結果。
#[derive(Debug)]
pub enum ConfigState {
    /// configが存在しない。default設定として扱う。
    Missing,
    /// 有効なconfig。version 1では未知のtop-level keyをwarningとして返す。
    Valid {
        config: Box<GlobalConfig>,
        warnings: Vec<Warning>,
    },
}

impl ConfigState {
    /// 保存済みの設定、または不在を意味するdefault設定。
    pub fn settings(self) -> GlobalConfig {
        match self {
            ConfigState::Missing => GlobalConfig::default(),
            ConfigState::Valid { config, .. } => *config,
        }
    }
}

/// YAMLの生表現。structへ変換する前にtop-level keyを検査する。
#[derive(Debug, Deserialize, Serialize)]
struct RawConfig {
    version: Option<i64>,
    language: Option<String>,
    #[serde(default)]
    files: Vec<RawFile>,
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

    // 欠落した`language`は未保存であり、不正な値とは別の状態である。
    let language = match raw.language {
        Some(value) => Some(Locale::parse_exact(&value).ok_or_else(|| {
            invalid_value(
                "language",
                format!("{value} is not one of {}", supported_language_list()),
            )
        })?),
        None => None,
    };

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
        config: Box::new(GlobalConfig { language, files }),
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
        language: config.language.map(|locale| locale.as_str().to_string()),
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

/// 表示言語だけをconfigへ保存する。
///
/// 既存configがあれば`language`の行だけを足すか差し替え、利用者が手で書いたコメント、
/// 空行、key順、`files`をそのまま残す。既知fieldだけで全文書を描き直さない。
pub fn save_language(location: &ConfigLocation, locale: Locale) -> Result<PathBuf> {
    ensure_config_dir(location)?;
    let path = location.config_file();
    let line = format!("language: {}", locale.as_str());

    let updated = match read_existing(&path)? {
        Some(text) => replace_language_line(&text, &line),
        None => render(&GlobalConfig {
            language: Some(locale),
            files: Vec::new(),
        }),
    };

    if paths::regular_file_exists(&path, PathScope::ConfigFile)? {
        crate::paths::atomic_replace(&path, &updated, PRIVATE_FILE_MODE)?;
    } else {
        atomic_create(&path, &updated, PRIVATE_FILE_MODE)?;
    }
    Ok(path)
}

/// 既存configの原文。存在しなければ`None`。
fn read_existing(path: &Path) -> Result<Option<String>> {
    if !paths::regular_file_exists(path, PathScope::ConfigFile)? {
        return Ok(None);
    }
    fs::read_to_string(path).map(Some).map_err(|error| {
        Error::new(
            ErrorId::ConfigUnreadable,
            msg!(
                "error-config-unreadable",
                path = paths::display(path),
                detail = error
            ),
        )
    })
}

/// top-levelの`language`行だけを差し替える。無ければ`version`行の直後へ足す。
fn replace_language_line(text: &str, line: &str) -> String {
    let declares_language = text.lines().any(|source| source.starts_with("language:"));
    let mut out = String::with_capacity(text.len() + line.len() + 1);
    let mut written = false;
    for source in text.lines() {
        if declares_language {
            if !written && source.starts_with("language:") {
                out.push_str(line);
                out.push('\n');
                written = true;
                continue;
            }
            out.push_str(source);
            out.push('\n');
            continue;
        }
        out.push_str(source);
        out.push('\n');
        if !written && source.starts_with("version:") {
            out.push_str(line);
            out.push('\n');
            written = true;
        }
    }
    if !written {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `~/.sbxm`を`0700`で検証または作成する。
pub fn ensure_config_dir(location: &ConfigLocation) -> Result<PathBuf> {
    let dir = location.dir();
    paths::ensure_private_dir(&dir, PRIVATE_DIR_MODE, PathScope::ConfigDir)?;
    Ok(dir)
}

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;

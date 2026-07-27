//! Global config `~/.sbxm/config.toml`。
//!
//! configはtoken、secret、runtime状態を保存しない。不正なconfigを自動修復せず、
//! `init`も上書きしない。

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Diagnostic, Error, ErrorId, Msg, Result, fail};
use crate::i18n::Locale;
use crate::msg;
use crate::paths::{
    self, AbsoluteBasePath, CONFIG_DIR_MODE, PRIVATE_FILE_MODE, SymlinkError, atomic_create,
    format_mode, permission_too_open,
};

/// このbuildが読み書きするconfigのversion。
pub const CONFIG_VERSION: u32 = 1;

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

    /// `~/.sbxm/config.toml`
    pub fn config_file(&self) -> PathBuf {
        self.dir().join("config.toml")
    }

    /// `~/.sbxm/init.lock`
    pub fn init_lock(&self) -> PathBuf {
        self.dir().join("init.lock")
    }

    /// `~/.sbxm/runtime`
    pub fn runtime_dir(&self) -> PathBuf {
        self.dir().join("runtime")
    }

    /// `~/.sbxm/runtime/daemon.lock`
    pub fn daemon_lock(&self) -> PathBuf {
        self.runtime_dir().join("daemon.lock")
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
        warnings: Vec<Msg>,
    },
}

/// TOMLの生表現。structへ変換する前にtop-level keyを検査する。
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
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::ConfigSymlink,
                msg!(
                    "security-config-symlink-description",
                    path = paths::display(&path)
                ),
            )
            .remediation(msg!(
                "security-config-symlink-remediation",
                path = paths::display(&path)
            )),
        ));
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
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::ConfigPermissionTooOpen,
                msg!(
                    "security-config-permission-description",
                    path = paths::display(&path),
                    observed = format_mode(mode)
                ),
            )
            .remediation(msg!(
                "security-config-permission-remediation",
                path = paths::display(&path),
                expected = format_mode(PRIVATE_FILE_MODE)
            )),
        ));
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
    let document: toml::Value = toml::from_str(text).map_err(|error| {
        Error::new(
            ErrorId::ConfigInvalidSyntax,
            msg!(
                "error-config-invalid-syntax",
                path = paths::display(path),
                detail = error.message()
            ),
        )
    })?;

    let mut warnings = Vec::new();
    if let Some(table) = document.as_table() {
        for key in table.keys() {
            if !KNOWN_TOP_LEVEL_KEYS.contains(&key.as_str()) {
                warnings.push(msg!(
                    "warning-config-unknown-key",
                    path = paths::display(path),
                    key = key
                ));
            }
        }
    }

    let raw: RawConfig = document.try_into().map_err(|error: toml::de::Error| {
        Error::new(
            ErrorId::ConfigInvalidSyntax,
            msg!(
                "error-config-invalid-syntax",
                path = paths::display(path),
                detail = error.message()
            ),
        )
    })?;

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

    // versionを最初に確定させ、未知versionを他の項目より前に診断する。
    match raw.version {
        Some(version) if version == i64::from(CONFIG_VERSION) => {}
        Some(version) => {
            return fail(
                ErrorId::ConfigUnknownVersion,
                msg!(
                    "error-config-unknown-version",
                    path = paths::display(path),
                    version = version,
                    supported = CONFIG_VERSION
                ),
            );
        }
        None => return Err(missing_field("version")),
    }

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

/// configをTOMLへ描画する。
pub fn render(config: &GlobalConfig) -> String {
    let mut out = String::new();
    out.push_str(&format!("version = {CONFIG_VERSION}\n"));
    out.push_str(&format!(
        "language = {}\n",
        toml_string(config.language.as_str())
    ));
    out.push_str(&format!(
        "base_path = {}\n",
        toml_string(&paths::display(config.base_path.as_path()))
    ));
    out.push_str("\n[git]\n");
    out.push_str(&format!(
        "user_name = {}\n",
        toml_string(&config.git.user_name)
    ));
    out.push_str(&format!(
        "user_email = {}\n",
        toml_string(&config.git.user_email)
    ));
    for declaration in &config.files {
        out.push_str("\n[[files]]\n");
        out.push_str(&format!(
            "source = {}\n",
            toml_string(&paths::display(declaration.source.as_path()))
        ));
        out.push_str(&format!(
            "destination = {}\n",
            toml_string(&paths::display(declaration.destination.as_path()))
        ));
    }
    out
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

/// `~/.sbxm`を`0700`で検証または作成する。
pub fn ensure_config_dir(location: &ConfigLocation) -> Result<PathBuf> {
    let dir = location.dir();
    paths::ensure_private_dir(&dir, CONFIG_DIR_MODE, SymlinkError::ConfigDir)?;
    Ok(dir)
}

/// configを新規作成する。既存configは上書きしない。
pub fn create(location: &ConfigLocation, config: &GlobalConfig) -> Result<PathBuf> {
    let path = location.config_file();
    atomic_create(&path, &render(config), PRIVATE_FILE_MODE)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location() -> (tempfile::TempDir, ConfigLocation) {
        let dir = tempfile::tempdir().expect("temporary home");
        let location = ConfigLocation::from_home(dir.path().to_path_buf());
        (dir, location)
    }

    fn valid_config_text(base_path: &Path) -> String {
        format!(
            r#"version = 1
language = "ja"
base_path = "{}"

[git]
user_name = "Example User"
user_email = "user@example.com"
"#,
            base_path.display()
        )
    }

    fn write_config(location: &ConfigLocation, text: &str) {
        let dir = location.dir();
        fs::create_dir_all(&dir).expect("create config dir");
        fs::set_permissions(&dir, fs::Permissions::from_mode(CONFIG_DIR_MODE)).expect("mode");
        let path = location.config_file();
        fs::write(&path, text).expect("write config");
        fs::set_permissions(&path, fs::Permissions::from_mode(PRIVATE_FILE_MODE)).expect("mode");
    }

    #[test]
    fn a_missing_configuration_is_reported_as_missing_rather_than_failing() {
        let (_dir, location) = location();
        assert!(matches!(
            load(&location).expect("missing is not an error"),
            ConfigState::Missing
        ));
    }

    #[test]
    fn configuration_paths_follow_the_documented_layout() {
        let location = ConfigLocation::from_home(PathBuf::from("/Users/example"));
        assert_eq!(location.dir(), PathBuf::from("/Users/example/.sbxm"));
        assert_eq!(
            location.config_file(),
            PathBuf::from("/Users/example/.sbxm/config.toml")
        );
        assert_eq!(
            location.init_lock(),
            PathBuf::from("/Users/example/.sbxm/init.lock")
        );
        assert_eq!(
            location.daemon_lock(),
            PathBuf::from("/Users/example/.sbxm/runtime/daemon.lock")
        );
    }

    #[test]
    fn a_valid_configuration_round_trips_through_render_and_load() {
        let (dir, location) = location();
        let base = dir.path().join("Projects");
        fs::create_dir_all(&base).unwrap();

        let config = GlobalConfig {
            language: Locale::Ja,
            base_path: AbsoluteBasePath::new(&base).unwrap(),
            git: GitIdentity {
                user_name: "Example User".into(),
                user_email: "user@example.com".into(),
            },
            files: vec![FileDeclaration {
                source: HostFileSource::new("/Users/example/.config/example/config.toml").unwrap(),
                destination: SandboxHomeRelativePath::new(".config/example/config.toml").unwrap(),
            }],
        };

        ensure_config_dir(&location).unwrap();
        create(&location, &config).unwrap();

        let ConfigState::Valid {
            config: loaded,
            warnings,
        } = load(&location).expect("the written configuration loads")
        else {
            panic!("the configuration must be present after create");
        };
        assert_eq!(*loaded, config);
        assert!(warnings.is_empty());
    }

    #[test]
    fn the_created_configuration_is_private_to_its_owner() {
        let (dir, location) = location();
        let base = dir.path().join("Projects");
        let config = GlobalConfig {
            language: Locale::En,
            base_path: AbsoluteBasePath::new(&base).unwrap(),
            git: GitIdentity {
                user_name: "Example User".into(),
                user_email: "user@example.com".into(),
            },
            files: Vec::new(),
        };
        ensure_config_dir(&location).unwrap();
        let path = create(&location, &config).unwrap();

        let file_mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600);
        let dir_mode = fs::metadata(location.dir()).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
    }

    #[test]
    fn creating_a_configuration_twice_does_not_overwrite_the_first() {
        let (dir, location) = location();
        let config = GlobalConfig {
            language: Locale::En,
            base_path: AbsoluteBasePath::new(&dir.path().join("Projects")).unwrap(),
            git: GitIdentity {
                user_name: "Example User".into(),
                user_email: "user@example.com".into(),
            },
            files: Vec::new(),
        };
        ensure_config_dir(&location).unwrap();
        create(&location, &config).unwrap();
        let error = create(&location, &config).expect_err("the second create must refuse");
        assert_eq!(error.first_id(), Some(ErrorId::TargetAppearedConcurrently));
    }

    #[test]
    fn invalid_syntax_is_reported_with_the_path() {
        let (_dir, location) = location();
        write_config(&location, "version = 1\nlanguage = \n");
        let error = load(&location).expect_err("broken TOML fails to load");
        assert_eq!(error.first_id(), Some(ErrorId::ConfigInvalidSyntax));
    }

    #[test]
    fn an_unknown_version_is_diagnosed_before_other_fields() {
        let (_dir, location) = location();
        write_config(&location, "version = 99\n");
        let error = load(&location).expect_err("unknown versions fail to load");
        assert_eq!(error.first_id(), Some(ErrorId::ConfigUnknownVersion));
    }

    #[test]
    fn missing_required_fields_are_named() {
        let (dir, location) = location();
        let cases = [
            ("version = 1\n", "language"),
            ("version = 1\nlanguage = \"en\"\n", "base_path"),
            (
                &format!(
                    "version = 1\nlanguage = \"en\"\nbase_path = \"{}\"\n",
                    dir.path().display()
                ),
                "git",
            ),
        ];
        for (text, _field) in cases {
            write_config(&location, text);
            let error = load(&location).expect_err("incomplete configurations fail");
            assert_eq!(error.first_id(), Some(ErrorId::ConfigMissingField));
        }
    }

    #[test]
    fn unknown_top_level_keys_are_warnings_in_version_1() {
        let (dir, location) = location();
        // top-levelのkeyとして解釈させるため、最初のtable headerより前へ置く。
        let text = valid_config_text(dir.path()).replace(
            "language = \"ja\"",
            "language = \"ja\"\nfuture_option = true",
        );
        write_config(&location, &text);

        let ConfigState::Valid { warnings, .. } = load(&location).expect("unknown keys still load")
        else {
            panic!("the configuration must load");
        };
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].id, "warning-config-unknown-key");
    }

    #[test]
    fn an_unsupported_language_is_rejected() {
        let (dir, location) = location();
        let text = valid_config_text(dir.path()).replace("\"ja\"", "\"fr\"");
        write_config(&location, &text);
        let error = load(&location).expect_err("unsupported languages fail");
        assert_eq!(error.first_id(), Some(ErrorId::ConfigInvalidValue));
    }

    #[test]
    fn a_relative_base_path_is_rejected() {
        let (dir, location) = location();
        let text = valid_config_text(dir.path()).replace(
            &format!("\"{}\"", dir.path().display()),
            "\"relative/projects\"",
        );
        write_config(&location, &text);
        let error = load(&location).expect_err("relative base paths fail");
        assert_eq!(error.first_id(), Some(ErrorId::BasePathNotAbsolute));
    }

    #[test]
    fn an_over_permissive_configuration_is_refused_and_not_repaired() {
        let (dir, location) = location();
        write_config(&location, &valid_config_text(dir.path()));
        let path = location.config_file();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let error = load(&location).expect_err("world-readable configurations are refused");
        assert_eq!(error.first_id(), Some(ErrorId::ConfigPermissionTooOpen));
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644, "sbxm must not repair permissions on its own");
    }

    #[test]
    fn a_symlinked_configuration_is_refused() {
        let (dir, location) = location();
        fs::create_dir_all(location.dir()).unwrap();
        let real = dir.path().join("real-config.toml");
        fs::write(&real, valid_config_text(dir.path())).unwrap();
        std::os::unix::fs::symlink(&real, location.config_file()).unwrap();

        let error = load(&location).expect_err("symlinked configurations are refused");
        assert_eq!(error.first_id(), Some(ErrorId::ConfigSymlink));
    }

    #[test]
    fn declared_file_sources_must_be_absolute() {
        let (dir, location) = location();
        let mut text = valid_config_text(dir.path());
        text.push_str("\n[[files]]\nsource = \"relative/file\"\ndestination = \".config/x\"\n");
        write_config(&location, &text);

        let error = load(&location).expect_err("relative sources are refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::FileDeclarationInvalidSource)
        );
    }

    #[test]
    fn declared_file_destinations_must_stay_under_the_sandbox_home() {
        let (dir, location) = location();
        for destination in ["/etc/passwd", "../outside", "nested/../../outside"] {
            let mut text = valid_config_text(dir.path());
            text.push_str(&format!(
                "\n[[files]]\nsource = \"/tmp/source\"\ndestination = \"{destination}\"\n"
            ));
            write_config(&location, &text);

            let error = load(&location).expect_err("{destination} must be refused");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::FileDeclarationInvalidDestination),
                "destination {destination} produced the wrong error"
            );
        }
    }

    #[test]
    fn git_identity_values_reject_empty_and_multi_line_input() {
        assert!(validate_git_identity_value("Example User").is_ok());
        assert_eq!(validate_git_identity_value(""), Err("detail-value-empty"));
        assert_eq!(
            validate_git_identity_value("   "),
            Err("detail-value-empty")
        );
        assert_eq!(
            validate_git_identity_value("Example\nUser"),
            Err("detail-value-has-newline")
        );
    }

    #[test]
    fn rendering_quotes_values_that_need_escaping() {
        let config = GlobalConfig {
            language: Locale::En,
            base_path: AbsoluteBasePath::from_standardized(PathBuf::from("/Users/ex ample")),
            git: GitIdentity {
                user_name: "Quote \" User".into(),
                user_email: "user@example.com".into(),
            },
            files: Vec::new(),
        };
        let rendered = render(&config);
        let reparsed: toml::Value = toml::from_str(&rendered).expect("rendered config is TOML");
        assert_eq!(reparsed["git"]["user_name"].as_str(), Some("Quote \" User"));
        assert_eq!(reparsed["base_path"].as_str(), Some("/Users/ex ample"));
    }
}

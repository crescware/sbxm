//! Error IDと診断の表現。
//!
//! 失敗理由はexit codeで分類せず、翻訳しない安定した英語error ID、選択言語による説明、
//! 対象、観測値、対処方法、必要な場合はredact済みの外部stderrで示す。

/// 公開契約となるexit code。
///
/// CLI parserを含む内部libraryの既定exit codeを公開契約へ透過しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// 成功、または仕様で成功と定めたno-op。
    Success,
    /// 通常error。引数不正、前提不足、設定・状態不正、外部command失敗、安全上の拒否を含む。
    Failure,
    /// Ctrl-CまたはEscによる対話キャンセル。
    Canceled,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        match self {
            ExitCode::Success => 0,
            ExitCode::Failure => 1,
            ExitCode::Canceled => 130,
        }
    }
}

/// 翻訳しない安定した英語error ID。
///
/// script側の分岐対象となる公開契約であり、locale、libraryのversion、
/// 外部commandのexit codeによって変化しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorId {
    // --- CLI parseと引数関係 ---
    InvalidArguments,
    UnknownArgument,
    InvalidValue,
    MissingRequiredArgument,
    MissingSubcommand,
    UnknownSubcommand,
    ConflictingArguments,
    InvalidLang,
    InitIncompleteOptions,
    WorktreesOutOfRange,
    WorktreesRequireDetach,
    ProjectArgumentRequired,
    StatusScopeRequired,
    NotImplemented,

    // --- Project識別子 ---
    InvalidProjectId,
    ReservedRepositoryName,

    // --- Global config ---
    ConfigMissing,
    ConfigUnreadable,
    ConfigInvalidSyntax,
    ConfigUnknownVersion,
    ConfigMissingField,
    ConfigInvalidValue,
    ConfigPermissionTooOpen,
    ConfigSymlink,
    ConfigDirPermissionTooOpen,
    ConfigDirSymlink,
    BasePathNotAbsolute,
    BasePathNotDirectory,
    BasePathNotWritable,
    BasePathEscapesRoot,
    FileDeclarationInvalidSource,
    FileDeclarationInvalidDestination,

    // --- Project metadata ---
    MetadataUnreadable,
    MetadataInvalidSyntax,
    MetadataUnknownVersion,
    MetadataMissingField,
    MetadataInvalidValue,
    MetadataPathMismatch,
    MetadataDuplicateProject,
    SandboxNameCollision,
    InvalidBranchName,
    TargetConfigurationMismatch,
    RebuildIntentPending,

    // --- Host clone ---
    HostCloneUnusable,

    // --- Image ---
    ImageUnusable,
    BuildContextNotEmpty,
    ArchiveUnusable,
    TemplateUnusable,
    SandboxUnusable,
    DeclaredFileUnusable,
    DeclaredFileConflict,

    // --- 案件のhost path ---
    ProjectPathSymlink,
    ProjectPathUnexpectedType,
    ProjectPathUnreadable,
    ProjectFilePermissionTooOpen,

    // --- 永続化 ---
    AtomicWriteFailed,
    TempFileLeftBehind,
    TargetAppearedConcurrently,
    TargetChangedConcurrently,
    LockTimeout,
    LockUnavailable,

    // --- 外部command ---
    ExternalCommandNotFound,
    ExternalCommandSpawnFailed,
    ExternalCommandFailed,
    ExternalCommandTimeout,
    ExternalOutputUnparseable,

    // --- Docker Sandboxes互換性 ---
    SbxVersionUnparseable,
    SbxVersionBelowMinimum,

    // --- Host環境診断 ---
    PlatformUnsupported,
    PlatformUnobservable,
    HostCommandMissing,
    DockerUnreachable,
    NetworkPolicyMismatch,
    NetworkPolicyUnobservable,
    DaemonUnobservable,
    DaemonSessionActive,
    DaemonSessionUnobservable,

    // --- init ---
    InitRequiresTty,
    GitIdentityInvalid,

    // --- 内部 ---
    MessageFormatFailed,
}

impl ErrorId {
    /// 安定した英語表記。翻訳せず、表示にもそのまま使う。
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorId::InvalidArguments => "invalid-arguments",
            ErrorId::UnknownArgument => "unknown-argument",
            ErrorId::InvalidValue => "invalid-value",
            ErrorId::MissingRequiredArgument => "missing-required-argument",
            ErrorId::MissingSubcommand => "missing-subcommand",
            ErrorId::UnknownSubcommand => "unknown-subcommand",
            ErrorId::ConflictingArguments => "conflicting-arguments",
            ErrorId::InvalidLang => "invalid-lang",
            ErrorId::InitIncompleteOptions => "init-incomplete-options",
            ErrorId::WorktreesOutOfRange => "worktrees-out-of-range",
            ErrorId::WorktreesRequireDetach => "worktrees-require-detach",
            ErrorId::ProjectArgumentRequired => "project-argument-required",
            ErrorId::StatusScopeRequired => "status-scope-required",
            ErrorId::NotImplemented => "not-implemented",

            ErrorId::InvalidProjectId => "invalid-project-id",
            ErrorId::ReservedRepositoryName => "reserved-repository-name",

            ErrorId::ConfigMissing => "config-missing",
            ErrorId::ConfigUnreadable => "config-unreadable",
            ErrorId::ConfigInvalidSyntax => "config-invalid-syntax",
            ErrorId::ConfigUnknownVersion => "config-unknown-version",
            ErrorId::ConfigMissingField => "config-missing-field",
            ErrorId::ConfigInvalidValue => "config-invalid-value",
            ErrorId::ConfigPermissionTooOpen => "config-permission-too-open",
            ErrorId::ConfigSymlink => "config-symlink",
            ErrorId::ConfigDirPermissionTooOpen => "config-dir-permission-too-open",
            ErrorId::ConfigDirSymlink => "config-dir-symlink",
            ErrorId::BasePathNotAbsolute => "base-path-not-absolute",
            ErrorId::BasePathNotDirectory => "base-path-not-directory",
            ErrorId::BasePathNotWritable => "base-path-not-writable",
            ErrorId::BasePathEscapesRoot => "base-path-escapes-root",
            ErrorId::FileDeclarationInvalidSource => "file-declaration-invalid-source",
            ErrorId::FileDeclarationInvalidDestination => "file-declaration-invalid-destination",

            ErrorId::MetadataUnreadable => "metadata-unreadable",
            ErrorId::MetadataInvalidSyntax => "metadata-invalid-syntax",
            ErrorId::MetadataUnknownVersion => "metadata-unknown-version",
            ErrorId::MetadataMissingField => "metadata-missing-field",
            ErrorId::MetadataInvalidValue => "metadata-invalid-value",
            ErrorId::MetadataPathMismatch => "metadata-path-mismatch",
            ErrorId::MetadataDuplicateProject => "metadata-duplicate-project",
            ErrorId::SandboxNameCollision => "sandbox-name-collision",
            ErrorId::InvalidBranchName => "invalid-branch-name",
            ErrorId::TargetConfigurationMismatch => "target-configuration-mismatch",
            ErrorId::RebuildIntentPending => "rebuild-intent-pending",

            ErrorId::HostCloneUnusable => "host-clone-unusable",

            ErrorId::ImageUnusable => "image-unusable",
            ErrorId::BuildContextNotEmpty => "build-context-not-empty",
            ErrorId::ArchiveUnusable => "archive-unusable",
            ErrorId::TemplateUnusable => "template-unusable",
            ErrorId::SandboxUnusable => "sandbox-unusable",
            ErrorId::DeclaredFileUnusable => "declared-file-unusable",
            ErrorId::DeclaredFileConflict => "declared-file-conflict",

            ErrorId::ProjectPathSymlink => "project-path-symlink",
            ErrorId::ProjectPathUnexpectedType => "project-path-unexpected-type",
            ErrorId::ProjectPathUnreadable => "project-path-unreadable",
            ErrorId::ProjectFilePermissionTooOpen => "project-file-permission-too-open",

            ErrorId::AtomicWriteFailed => "atomic-write-failed",
            ErrorId::TempFileLeftBehind => "temp-file-left-behind",
            ErrorId::TargetAppearedConcurrently => "target-appeared-concurrently",
            ErrorId::TargetChangedConcurrently => "target-changed-concurrently",
            ErrorId::LockTimeout => "lock-timeout",
            ErrorId::LockUnavailable => "lock-unavailable",

            ErrorId::ExternalCommandNotFound => "external-command-not-found",
            ErrorId::ExternalCommandSpawnFailed => "external-command-spawn-failed",
            ErrorId::ExternalCommandFailed => "external-command-failed",
            ErrorId::ExternalCommandTimeout => "external-command-timeout",
            ErrorId::ExternalOutputUnparseable => "external-output-unparseable",

            ErrorId::SbxVersionUnparseable => "sbx-version-unparseable",
            ErrorId::SbxVersionBelowMinimum => "sbx-version-below-minimum",

            ErrorId::PlatformUnsupported => "platform-unsupported",
            ErrorId::PlatformUnobservable => "platform-unobservable",
            ErrorId::HostCommandMissing => "host-command-missing",
            ErrorId::DockerUnreachable => "docker-unreachable",
            ErrorId::NetworkPolicyMismatch => "network-policy-mismatch",
            ErrorId::NetworkPolicyUnobservable => "network-policy-unobservable",
            ErrorId::DaemonUnobservable => "daemon-unobservable",
            ErrorId::DaemonSessionActive => "daemon-session-active",
            ErrorId::DaemonSessionUnobservable => "daemon-session-unobservable",

            ErrorId::InitRequiresTty => "init-requires-tty",
            ErrorId::GitIdentityInvalid => "git-identity-invalid",

            ErrorId::MessageFormatFailed => "message-format-failed",
        }
    }
}

impl std::fmt::Display for ErrorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// FTL message IDと、その引数。
///
/// 利用者向け文字列はすべてFTL resourceから生成するため、診断は表示文字列ではなく
/// message参照として持ち回る。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg {
    pub id: &'static str,
    pub args: Vec<(&'static str, String)>,
}

impl Msg {
    pub fn new(id: &'static str) -> Self {
        Msg {
            id,
            args: Vec::new(),
        }
    }

    pub fn with(mut self, key: &'static str, value: impl std::fmt::Display) -> Self {
        self.args.push((key, value.to_string()));
        self
    }
}

/// FTL messageを組み立てる。
///
/// ```ignore
/// msg!("config-missing");
/// msg!("config-invalid-syntax", path = display_path, detail = err);
/// ```
#[macro_export]
macro_rules! msg {
    ($id:expr) => {
        $crate::error::Msg::new($id)
    };
    ($id:expr, $($key:ident = $value:expr),+ $(,)?) => {
        $crate::error::Msg::new($id)
            $(.with(stringify!($key), &$value))+
    };
}

/// 外部commandの失敗を、翻訳せず原文のまま持つ。
///
/// stderrはFTL placeholderへ埋め込まず、localized説明とは別blockで表示する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFailure {
    pub program: String,
    /// secret値を含まないことが保証されたargumentだけを保持する。
    pub safe_args: Vec<String>,
    /// 実行時の作業directory。指定していない場合は`None`。
    pub working_dir: Option<std::path::PathBuf>,
    /// 外部commandのexit statusを原値のまま示す文字列。
    pub exit_status: String,
    pub stderr: Vec<u8>,
    /// stderrをUTF-8として解釈する際にlossy変換が発生したか。
    pub stderr_lossy: bool,
}

impl ExternalFailure {
    /// 表示用のstderr。原文のbyte列をlossyに変換するが、変換の有無は別途診断する。
    #[cfg(test)]
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

/// 1件の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub id: ErrorId,
    pub description: Msg,
    pub remediation: Option<Msg>,
    pub external: Option<ExternalFailure>,
}

impl Diagnostic {
    pub fn new(id: ErrorId, description: Msg) -> Self {
        Diagnostic {
            id,
            description,
            remediation: None,
            external: None,
        }
    }

    pub fn remediation(mut self, remediation: Msg) -> Self {
        self.remediation = Some(remediation);
        self
    }

    pub fn external(mut self, external: ExternalFailure) -> Self {
        self.external = Some(external);
        self
    }
}

/// 1件以上の診断、または対話キャンセル。
///
/// 複数種類のerrorがあってもexit codeは`1`とし、個々のerror IDと診断をすべて表示する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Diagnostics(Vec<Diagnostic>),
    /// Ctrl-CまたはEscによる対話キャンセル。何も変更していないことを表す。
    Canceled,
}

impl Error {
    pub fn new(id: ErrorId, description: Msg) -> Self {
        Error::Diagnostics(vec![Diagnostic::new(id, description)])
    }

    pub fn single(diagnostic: Diagnostic) -> Self {
        Error::Diagnostics(vec![diagnostic])
    }

    #[cfg(test)]
    pub fn many(diagnostics: Vec<Diagnostic>) -> Self {
        Error::Diagnostics(diagnostics)
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Error::Diagnostics(_) => ExitCode::Failure,
            Error::Canceled => ExitCode::Canceled,
        }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Error::Diagnostics(items) => items,
            Error::Canceled => &[],
        }
    }

    /// 最初の診断のerror ID。testと呼び出し側の分岐に使う。
    #[cfg(test)]
    pub fn first_id(&self) -> Option<ErrorId> {
        self.diagnostics().first().map(|d| d.id)
    }

    #[cfg(test)]
    pub fn contains(&self, id: ErrorId) -> bool {
        self.diagnostics().iter().any(|d| d.id == id)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// 診断を1件だけ持つ`Err`を作る短縮形。
pub fn fail<T>(id: ErrorId, description: Msg) -> Result<T> {
    Err(Error::new(id, description))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_the_published_contract() {
        assert_eq!(ExitCode::Success.as_i32(), 0);
        assert_eq!(ExitCode::Failure.as_i32(), 1);
        assert_eq!(ExitCode::Canceled.as_i32(), 130);
    }

    #[test]
    fn cancellation_maps_to_130_and_everything_else_to_1() {
        assert_eq!(Error::Canceled.exit_code(), ExitCode::Canceled);
        assert_eq!(
            Error::new(ErrorId::ConfigMissing, msg!("config-missing")).exit_code(),
            ExitCode::Failure
        );
    }

    #[test]
    fn error_ids_are_stable_kebab_case_ascii() {
        for id in all_error_ids() {
            let text = id.as_str();
            assert!(!text.is_empty(), "{id:?} has an empty error ID");
            assert!(
                text.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "{text} is not stable lowercase kebab-case"
            );
            assert!(
                !text.starts_with('-') && !text.ends_with('-'),
                "{text} must not start or end with a hyphen"
            );
        }
    }

    #[test]
    fn error_ids_are_unique() {
        let mut seen: Vec<&'static str> = all_error_ids().iter().map(|id| id.as_str()).collect();
        let total = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), total, "duplicate error ID strings detected");
    }

    #[test]
    fn msg_macro_collects_named_arguments() {
        let message = msg!("config-invalid-syntax", path = "/tmp/x", detail = 42);
        assert_eq!(message.id, "config-invalid-syntax");
        assert_eq!(
            message.args,
            vec![("path", "/tmp/x".to_string()), ("detail", "42".to_string())]
        );
    }

    #[test]
    fn external_failure_keeps_raw_stderr() {
        let failure = ExternalFailure {
            program: "sbx".into(),
            safe_args: vec!["ls".into()],
            working_dir: None,
            exit_status: "exit status: 2".into(),
            stderr: b"boom\n".to_vec(),
            stderr_lossy: false,
        };
        assert_eq!(failure.stderr_text(), "boom\n");
    }

    /// `ErrorId`の全variant。新しいIDを追加したらここにも追加する。
    pub(super) fn all_error_ids() -> Vec<ErrorId> {
        vec![
            ErrorId::InvalidArguments,
            ErrorId::UnknownArgument,
            ErrorId::InvalidValue,
            ErrorId::MissingRequiredArgument,
            ErrorId::MissingSubcommand,
            ErrorId::UnknownSubcommand,
            ErrorId::ConflictingArguments,
            ErrorId::InvalidLang,
            ErrorId::InitIncompleteOptions,
            ErrorId::WorktreesOutOfRange,
            ErrorId::WorktreesRequireDetach,
            ErrorId::ProjectArgumentRequired,
            ErrorId::StatusScopeRequired,
            ErrorId::NotImplemented,
            ErrorId::InvalidProjectId,
            ErrorId::ReservedRepositoryName,
            ErrorId::ConfigMissing,
            ErrorId::ConfigUnreadable,
            ErrorId::ConfigInvalidSyntax,
            ErrorId::ConfigUnknownVersion,
            ErrorId::ConfigMissingField,
            ErrorId::ConfigInvalidValue,
            ErrorId::ConfigPermissionTooOpen,
            ErrorId::ConfigSymlink,
            ErrorId::ConfigDirPermissionTooOpen,
            ErrorId::ConfigDirSymlink,
            ErrorId::BasePathNotAbsolute,
            ErrorId::BasePathNotDirectory,
            ErrorId::BasePathNotWritable,
            ErrorId::BasePathEscapesRoot,
            ErrorId::FileDeclarationInvalidSource,
            ErrorId::FileDeclarationInvalidDestination,
            ErrorId::MetadataUnreadable,
            ErrorId::MetadataInvalidSyntax,
            ErrorId::TargetConfigurationMismatch,
            ErrorId::RebuildIntentPending,
            ErrorId::MetadataUnknownVersion,
            ErrorId::MetadataMissingField,
            ErrorId::MetadataInvalidValue,
            ErrorId::MetadataPathMismatch,
            ErrorId::MetadataDuplicateProject,
            ErrorId::SandboxNameCollision,
            ErrorId::InvalidBranchName,
            ErrorId::HostCloneUnusable,
            ErrorId::ImageUnusable,
            ErrorId::BuildContextNotEmpty,
            ErrorId::ArchiveUnusable,
            ErrorId::TemplateUnusable,
            ErrorId::SandboxUnusable,
            ErrorId::DeclaredFileUnusable,
            ErrorId::DeclaredFileConflict,
            ErrorId::ProjectPathSymlink,
            ErrorId::ProjectPathUnexpectedType,
            ErrorId::ProjectPathUnreadable,
            ErrorId::ProjectFilePermissionTooOpen,
            ErrorId::AtomicWriteFailed,
            ErrorId::TempFileLeftBehind,
            ErrorId::TargetAppearedConcurrently,
            ErrorId::TargetChangedConcurrently,
            ErrorId::LockTimeout,
            ErrorId::LockUnavailable,
            ErrorId::ExternalCommandNotFound,
            ErrorId::ExternalCommandSpawnFailed,
            ErrorId::ExternalCommandFailed,
            ErrorId::ExternalCommandTimeout,
            ErrorId::ExternalOutputUnparseable,
            ErrorId::SbxVersionUnparseable,
            ErrorId::SbxVersionBelowMinimum,
            ErrorId::PlatformUnsupported,
            ErrorId::PlatformUnobservable,
            ErrorId::HostCommandMissing,
            ErrorId::DockerUnreachable,
            ErrorId::NetworkPolicyMismatch,
            ErrorId::NetworkPolicyUnobservable,
            ErrorId::DaemonUnobservable,
            ErrorId::DaemonSessionActive,
            ErrorId::DaemonSessionUnobservable,
            ErrorId::InitRequiresTty,
            ErrorId::GitIdentityInvalid,
            ErrorId::MessageFormatFailed,
        ]
    }
}

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
    WorktreesNotReducible,
    ApplyScopeRequired,
    ProjectArgumentRequired,
    StatusScopeRequired,

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
    ConfigNotOwned,
    ConfigDirPermissionTooOpen,
    ConfigDirSymlink,
    ConfigDirNotOwned,
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
    SandboxIdentityMismatch,
    GithubSecretMissing,
    SandboxSecretNotApplied,
    SandboxRepositoryUnusable,
    StartRefUnresolved,
    ProjectNotManaged,
    NoManagedProjects,
    SelectionUnresolved,
    SandboxNotCreated,
    SandboxNotRunning,
    SandboxStillRunning,
    SandboxStillPresent,
    RebuildGenerationMissing,
    DestroyNotConfirmed,
    SandboxCheckUnobservable,
    GlobalScopeUnobservable,
    SshAgentExposed,
    UnsavedWork,
    WorktreeOutsideRepository,
    UnmanagedWorktreePresent,
    SbxLoginMissing,
    SbxLoginUnobservable,
    RemoteSshUnconfigured,
    RemoteSshUnobservable,

    // --- 案件のhost path ---
    ProjectPathSymlink,
    ProjectPathUnexpectedType,
    ProjectPathUnreadable,
    ProjectPathNotOwned,
    ProjectFilePermissionTooOpen,

    // --- 永続化 ---
    AtomicWriteFailed,
    TempFileLeftBehind,
    CleanupFailed,
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

    // --- init ---
    InitRequiresTty,
    PromptUnreadable,
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
            ErrorId::WorktreesNotReducible => "worktrees-not-reducible",
            ErrorId::ApplyScopeRequired => "apply-scope-required",
            ErrorId::ProjectArgumentRequired => "project-argument-required",
            ErrorId::StatusScopeRequired => "status-scope-required",

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
            ErrorId::ConfigNotOwned => "config-not-owned",
            ErrorId::ConfigDirPermissionTooOpen => "config-dir-permission-too-open",
            ErrorId::ConfigDirSymlink => "config-dir-symlink",
            ErrorId::ConfigDirNotOwned => "config-dir-not-owned",
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
            ErrorId::SandboxIdentityMismatch => "sandbox-identity-mismatch",
            ErrorId::GithubSecretMissing => "github-secret-missing",
            ErrorId::SandboxSecretNotApplied => "sandbox-secret-not-applied",
            ErrorId::SandboxRepositoryUnusable => "sandbox-repository-unusable",
            ErrorId::StartRefUnresolved => "start-ref-unresolved",
            ErrorId::ProjectNotManaged => "project-not-managed",
            ErrorId::NoManagedProjects => "no-managed-projects",
            ErrorId::SelectionUnresolved => "selection-unresolved",
            ErrorId::SandboxNotCreated => "sandbox-not-created",
            ErrorId::SandboxNotRunning => "sandbox-not-running",
            ErrorId::SandboxStillRunning => "sandbox-still-running",
            ErrorId::SandboxStillPresent => "sandbox-still-present",
            ErrorId::RebuildGenerationMissing => "rebuild-generation-missing",
            ErrorId::DestroyNotConfirmed => "destroy-not-confirmed",
            ErrorId::SandboxCheckUnobservable => "sandbox-check-unobservable",
            ErrorId::GlobalScopeUnobservable => "global-scope-unobservable",
            ErrorId::SshAgentExposed => "ssh-agent-exposed",
            ErrorId::UnsavedWork => "unsaved-work",
            ErrorId::WorktreeOutsideRepository => "worktree-outside-repository",
            ErrorId::UnmanagedWorktreePresent => "unmanaged-worktree-present",
            ErrorId::SbxLoginMissing => "sbx-login-missing",
            ErrorId::SbxLoginUnobservable => "sbx-login-unobservable",
            ErrorId::RemoteSshUnconfigured => "remote-ssh-unconfigured",
            ErrorId::RemoteSshUnobservable => "remote-ssh-unobservable",

            ErrorId::ProjectPathSymlink => "project-path-symlink",
            ErrorId::ProjectPathUnexpectedType => "project-path-unexpected-type",
            ErrorId::ProjectPathUnreadable => "project-path-unreadable",
            ErrorId::ProjectPathNotOwned => "project-path-not-owned",
            ErrorId::ProjectFilePermissionTooOpen => "project-file-permission-too-open",

            ErrorId::AtomicWriteFailed => "atomic-write-failed",
            ErrorId::TempFileLeftBehind => "temp-file-left-behind",
            ErrorId::CleanupFailed => "cleanup-failed",
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

            ErrorId::InitRequiresTty => "init-requires-tty",
            ErrorId::PromptUnreadable => "prompt-unreadable",
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

    /// 指定したerror IDを含むか。呼び出し側の分岐に使う。
    pub fn contains_id(&self, id: ErrorId) -> bool {
        self.diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.id == id)
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
#[path = "error_test.rs"]
mod error_test;

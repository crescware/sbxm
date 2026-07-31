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

/// `ErrorId`のvariantと、その安定した英語表記を1箇所で宣言する。
///
/// 片方だけを足せる形にすると、表記のないIDも、testの一覧から漏れるIDも作れて
/// しまう。同じ1行で宣言し、testが辿る`ALL`も同じ宣言から組み立てる。
macro_rules! error_ids {
    ($($variant:ident => $text:literal),+ $(,)?) => {
        /// 翻訳しない安定した英語error ID。
        ///
        /// script側の分岐対象となる公開契約であり、locale、libraryのversion、
        /// 外部commandのexit codeによって変化しない。
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub enum ErrorId {
            $($variant),+
        }

        impl ErrorId {
            /// 安定した英語表記。翻訳せず、表示にもそのまま使う。
            pub fn as_str(self) -> &'static str {
                match self {
                    $(ErrorId::$variant => $text),+
                }
            }

            /// 全variant。宣言から組み立てるため、追加し忘れが起きない。
            #[cfg(test)]
            pub const ALL: &'static [ErrorId] = &[$(ErrorId::$variant),+];
        }
    };
}

error_ids! {
    // --- CLI parseと引数関係 ---
    InvalidArguments => "invalid-arguments",
    UnknownArgument => "unknown-argument",
    InvalidValue => "invalid-value",
    MissingRequiredArgument => "missing-required-argument",
    MissingSubcommand => "missing-subcommand",
    UnknownSubcommand => "unknown-subcommand",
    ConflictingArguments => "conflicting-arguments",
    InvalidLang => "invalid-lang",
    WorktreesOutOfRange => "worktrees-out-of-range",
    WorktreesRequireDetach => "worktrees-require-detach",
    WorktreesNotReducible => "worktrees-not-reducible",
    ApplyScopeRequired => "apply-scope-required",
    ProjectArgumentRequired => "project-argument-required",
    StatusScopeRequired => "status-scope-required",

    // --- Project識別子 ---
    InvalidProjectId => "invalid-project-id",
    ReservedRepositoryName => "reserved-repository-name",
    InvalidCloneUrl => "invalid-clone-url",

    // --- Global config ---
    ConfigUnreadable => "config-unreadable",
    ConfigInvalidSyntax => "config-invalid-syntax",
    ConfigUnknownVersion => "config-unknown-version",
    ConfigMissingField => "config-missing-field",
    ConfigInvalidValue => "config-invalid-value",
    ConfigPermissionTooOpen => "config-permission-too-open",
    ConfigSymlink => "config-symlink",
    ConfigNotOwned => "config-not-owned",
    ConfigDirPermissionTooOpen => "config-dir-permission-too-open",
    ConfigDirSymlink => "config-dir-symlink",
    ConfigDirNotOwned => "config-dir-not-owned",
    GlobalStateUnusable => "global-state-unusable",
    FileDeclarationInvalidSource => "file-declaration-invalid-source",
    FileDeclarationInvalidDestination => "file-declaration-invalid-destination",

    // --- Global registry ---
    RegistryUnreadable => "registry-unreadable",
    RegistryInvalidSyntax => "registry-invalid-syntax",
    RegistryUnknownVersion => "registry-unknown-version",
    RegistryMissingField => "registry-missing-field",
    RegistryInvalidValue => "registry-invalid-value",
    RegistryDuplicateProject => "registry-duplicate-project",
    RegistryDuplicateRoot => "registry-duplicate-root",
    RegistryEntryMismatch => "registry-entry-mismatch",

    // --- Project metadata ---
    MetadataUnreadable => "metadata-unreadable",
    MetadataInvalidSyntax => "metadata-invalid-syntax",
    MetadataUnknownVersion => "metadata-unknown-version",
    MetadataMissingField => "metadata-missing-field",
    MetadataInvalidValue => "metadata-invalid-value",
    SandboxNameCollision => "sandbox-name-collision",
    InvalidBranchName => "invalid-branch-name",
    TargetConfigurationMismatch => "target-configuration-mismatch",
    RebuildIntentPending => "rebuild-intent-pending",

    // --- Host clone ---
    HostCloneUnusable => "host-clone-unusable",

    // --- Image ---
    ImageUnusable => "image-unusable",
    BuildContextNotEmpty => "build-context-not-empty",
    ArchiveUnusable => "archive-unusable",
    TemplateUnusable => "template-unusable",
    SandboxUnusable => "sandbox-unusable",
    DeclaredFileUnusable => "declared-file-unusable",
    DeclaredFileConflict => "declared-file-conflict",
    SandboxIdentityMismatch => "sandbox-identity-mismatch",
    GithubSecretMissing => "github-secret-missing",
    SandboxSecretNotApplied => "sandbox-secret-not-applied",
    SecretStillRegistered => "secret-still-registered",
    SandboxRepositoryUnusable => "sandbox-repository-unusable",
    StartRefUnresolved => "start-ref-unresolved",
    ProjectNotManaged => "project-not-managed",
    ProjectIncomplete => "project-incomplete",
    ProjectInconsistent => "project-inconsistent",
    NoManagedProjects => "no-managed-projects",
    SelectionUnresolved => "selection-unresolved",
    SandboxNotCreated => "sandbox-not-created",
    SandboxNotRunning => "sandbox-not-running",
    SandboxStillRunning => "sandbox-still-running",
    SandboxStillPresent => "sandbox-still-present",
    RebuildGenerationMissing => "rebuild-generation-missing",
    DestroyNotConfirmed => "destroy-not-confirmed",
    SandboxCheckUnobservable => "sandbox-check-unobservable",
    GlobalScopeUnobservable => "global-scope-unobservable",
    SshAgentExposed => "ssh-agent-exposed",
    UnsavedWork => "unsaved-work",
    WorktreeOutsideRepository => "worktree-outside-repository",
    UnmanagedWorktreePresent => "unmanaged-worktree-present",
    SbxLoginMissing => "sbx-login-missing",
    SbxLoginUnobservable => "sbx-login-unobservable",
    RemoteSshUnconfigured => "remote-ssh-unconfigured",
    RemoteSshUnobservable => "remote-ssh-unobservable",

    // --- 案件のhost path ---
    ProjectPathCollision => "project-path-collision",
    WorkingDirectoryUnusable => "working-directory-unusable",
    ProjectPathSymlink => "project-path-symlink",
    ProjectPathUnexpectedType => "project-path-unexpected-type",
    ProjectPathUnreadable => "project-path-unreadable",
    ProjectPathNotOwned => "project-path-not-owned",
    ProjectFilePermissionTooOpen => "project-file-permission-too-open",

    // --- 永続化 ---
    AtomicWriteFailed => "atomic-write-failed",
    TempFileLeftBehind => "temp-file-left-behind",
    CleanupFailed => "cleanup-failed",
    TargetAppearedConcurrently => "target-appeared-concurrently",
    TargetChangedConcurrently => "target-changed-concurrently",
    LockTimeout => "lock-timeout",
    LockUnavailable => "lock-unavailable",

    // --- 外部command ---
    ExternalCommandNotFound => "external-command-not-found",
    ExternalCommandSpawnFailed => "external-command-spawn-failed",
    ExternalCommandFailed => "external-command-failed",
    ExternalCommandTimeout => "external-command-timeout",
    ExternalOutputUnparseable => "external-output-unparseable",

    // --- Docker Sandboxes互換性 ---
    SbxVersionUnparseable => "sbx-version-unparseable",
    SbxVersionBelowMinimum => "sbx-version-below-minimum",

    // --- Host環境診断 ---
    PlatformUnsupported => "platform-unsupported",
    PlatformUnobservable => "platform-unobservable",
    HostCommandMissing => "host-command-missing",
    DockerUnreachable => "docker-unreachable",
    NetworkPolicyMismatch => "network-policy-mismatch",
    NetworkPolicyUnobservable => "network-policy-unobservable",
    DaemonUnobservable => "daemon-unobservable",

    // --- 対話 ---
    PromptUnreadable => "prompt-unreadable",
    GitIdentityUnavailable => "git-identity-unavailable",

    // --- 内部 ---
    MessageFormatFailed => "message-format-failed",
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
    /// 説明と、実行するcommandを分けて持つ対処方法。
    ///
    /// 説明文へcommandを埋め込むと、command行を独立させるという表示の不変条件を
    /// rendererが守れず、翻訳者がcommandの綴りを預かることにもなる。
    pub remediation: Option<crate::ui::Remediation>,
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

    pub fn remediation(mut self, remediation: impl Into<crate::ui::Remediation>) -> Self {
        self.remediation = Some(remediation.into());
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

    /// 最初の診断のerror ID。testの検証に使う。
    #[cfg(test)]
    pub fn first_id(&self) -> Option<ErrorId> {
        self.diagnostics().first().map(|d| d.id)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// 診断を1件だけ持つ`Err`を作る短縮形。
pub fn fail<T>(id: ErrorId, description: Msg) -> Result<T> {
    Err(Error::new(id, description))
}

/// versionを持つ文書の、versionの読み方。
///
/// 文書ごとに1度宣言し、parseの先頭で`require`を呼ぶ。
pub struct DocumentVersion {
    /// 読める唯一のversion。
    pub supported: u32,
    /// 未知versionのerror ID。
    pub unknown: ErrorId,
    /// 未知versionのFTL message ID。
    pub unknown_message: &'static str,
}

impl DocumentVersion {
    /// versionを最初に確定させ、未知versionを他の項目より前に診断する。
    ///
    /// `path`は表示用に整えたもの、`missing`はversion欄自体が無い場合の診断とする。
    pub fn require(
        &self,
        found: Option<i64>,
        path: &str,
        missing: impl FnOnce() -> Error,
    ) -> Result<()> {
        match found {
            Some(version) if version == i64::from(self.supported) => Ok(()),
            Some(version) => fail(
                self.unknown,
                msg!(
                    self.unknown_message,
                    path = path,
                    version = version,
                    supported = self.supported
                ),
            ),
            None => Err(missing()),
        }
    }
}

#[cfg(test)]
#[path = "error_test.rs"]
mod error_test;

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
        }

        /// 宣言した全variant。testが`ErrorId::ALL`を組み立てるために使う。
        ///
        /// 一覧そのものを本番fileへ置くとcoverageが数える母集団へ入るため、宣言から
        /// 組み立てる手段だけをここへ残す。`#[macro_export]`はcrate外へ出すためではなく、
        /// 本番buildで未使用のmacroが警告にならないようにするために付ける。
        #[macro_export]
        macro_rules! declared_error_ids {
            () => { &[$(ErrorId::$variant),+] };
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
    ConfigNotRewritable => "config-not-rewritable",
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
    ExternalCommandOutputUnreadable => "external-command-output-unreadable",
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
    // 名義の宣言が片方だけである。
    GitIdentityIncomplete => "git-identity-incomplete",
    // 訊く手段も、保存済みの既定も、宣言も無い。
    GitIdentityUndecidable => "git-identity-undecidable",

    // --- 内部 ---
    MessageFormatFailed => "message-format-failed",
    DocumentRenderFailed => "document-render-failed",
}

impl std::fmt::Display for ErrorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "error_id_test.rs"]
mod error_id_test;

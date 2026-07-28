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
        ErrorId::ConfigNotOwned,
        ErrorId::ConfigDirPermissionTooOpen,
        ErrorId::ConfigDirSymlink,
        ErrorId::ConfigDirNotOwned,
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
        ErrorId::SandboxIdentityMismatch,
        ErrorId::GithubSecretMissing,
        ErrorId::SandboxSecretNotApplied,
        ErrorId::SandboxRepositoryUnusable,
        ErrorId::StartRefUnresolved,
        ErrorId::ProjectNotManaged,
        ErrorId::NoManagedProjects,
        ErrorId::SelectionUnresolved,
        ErrorId::SandboxNotCreated,
        ErrorId::SandboxNotRunning,
        ErrorId::SandboxStillRunning,
        ErrorId::SandboxStillPresent,
        ErrorId::RebuildGenerationMissing,
        ErrorId::DestroyNotConfirmed,
        ErrorId::SandboxCheckUnobservable,
        ErrorId::GlobalScopeUnobservable,
        ErrorId::SshAgentExposed,
        ErrorId::UnsavedWork,
        ErrorId::WorktreeOutsideRepository,
        ErrorId::UnmanagedWorktreePresent,
        ErrorId::SbxLoginMissing,
        ErrorId::SbxLoginUnobservable,
        ErrorId::RemoteSshUnconfigured,
        ErrorId::RemoteSshUnobservable,
        ErrorId::ProjectPathSymlink,
        ErrorId::ProjectPathUnexpectedType,
        ErrorId::ProjectPathUnreadable,
        ErrorId::ProjectPathNotOwned,
        ErrorId::ProjectFilePermissionTooOpen,
        ErrorId::AtomicWriteFailed,
        ErrorId::TempFileLeftBehind,
        ErrorId::CleanupFailed,
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
        ErrorId::InitRequiresTty,
        ErrorId::PromptUnreadable,
        ErrorId::GitIdentityInvalid,
        ErrorId::MessageFormatFailed,
    ]
}

locale-name = English

cli-about = Manage Docker Sandboxes per project: set up, connect, inspect and tear down.
cli-heading-usage = Usage:
cli-heading-commands = Commands:
cli-heading-options = Options:
cli-heading-arguments = Arguments:
cli-lang-help = Display language for this run ({ $supported })
cli-help-help = Print help
cli-version-help = Print version

cli-init-about = Create the global configuration for sbxm
cli-init-base-path-help = Absolute path of the directory that holds project directories
cli-init-git-user-name-help = Git user.name applied inside sandboxes
cli-init-git-user-email-help = Git user.email applied inside sandboxes

cli-add-about = Register a GitHub repository and build its sandbox
cli-add-project-help = Target project as owner/repository
cli-add-worktrees-help = Number of managed worktrees to create (1-32)
cli-add-detach-help = Remote branch every managed worktree starts from, in detached mode

cli-sync-files-about = Re-place the files declared in the global configuration into a running sandbox
cli-sync-files-project-help = Target project as owner/repository

cli-rebuild-about = Apply the edited Dockerfile by recreating the sandbox
cli-rebuild-project-help = Target project as owner/repository

cli-open-about = Start the sandbox if needed and connect to it over SSH
cli-open-project-help = Target project as owner/repository

cli-stop-about = Stop running sandboxes
cli-stop-project-help = Target projects as owner/repository

cli-ls-about = List managed projects and their sandbox state

cli-status-about = Diagnose the host environment or a single project, read-only
cli-status-project-help = Target project as owner/repository
cli-status-global-help = Diagnose the host and global environment instead of a project

cli-destroy-about = Delete the sandbox and drop sbxm management data for a project
cli-destroy-project-help = Target project as owner/repository
cli-destroy-force-help = Skip data-protection and active-session checks, then delete

error-invalid-arguments = The arguments could not be interpreted.
error-unknown-argument = Unknown argument: { $argument }
error-invalid-value = Value { $value } is not valid for { $argument }.
error-missing-required-argument = Required argument is missing: { $argument }
error-missing-subcommand = A command is required.
error-unknown-subcommand = Unknown command: { $subcommand }
error-conflicting-arguments = These arguments cannot be used together: { $arguments }
error-invalid-lang = Value { $value } is not a supported display language. Supported values: { $supported }
error-init-incomplete-options = Interactive mode takes none of these options and option mode takes all of them. Missing: { $missing }
error-worktrees-out-of-range = The number of managed worktrees must be between { $minimum } and { $maximum }. Observed: { $value }
error-worktrees-require-detach = Creating more than one managed worktree requires an explicit start branch.
error-project-argument-required = { $command } needs an explicit owner/repository argument when the session is not interactive.
error-status-scope-required = Specify exactly one scope: either the global environment or one owner/repository.
error-not-implemented = { $command } is not implemented in this build.

usage-hint = { $usage }

error-invalid-project-id = { $value } is not a valid owner/repository identifier.
error-reserved-repository-name = { $value } is reserved and cannot be used as a repository name.

error-config-missing = No global configuration was found at { $path }.
error-config-unreadable = The global configuration at { $path } could not be read: { $detail }
error-config-invalid-syntax = The global configuration at { $path } is not valid TOML: { $detail }
error-config-unknown-version = The global configuration at { $path } declares version { $version }, but this build supports { $supported }.
error-config-missing-field = The global configuration at { $path } is missing the required field { $field }.
error-config-invalid-value = Field { $field } in { $path } is not valid: { $detail }
error-base-path-not-absolute = The base path { $path } is not absolute.
error-base-path-not-directory = The base path { $path } exists but is not a directory.
error-base-path-not-writable = The base path { $path } is not writable by the current user.
error-file-declaration-invalid-source = Declared file { $index } has an invalid source { $source }: { $detail }
error-file-declaration-invalid-destination = Declared file { $index } has an invalid destination { $destination }: { $detail }
warning-config-unknown-key = Unknown key { $key } in { $path } was ignored.

error-metadata-unreadable = The project metadata at { $path } could not be read: { $detail }
error-metadata-invalid-syntax = The project metadata at { $path } is not valid TOML: { $detail }
error-metadata-unknown-version = The project metadata at { $path } declares version { $version }, but this build supports { $supported }.
error-metadata-missing-field = The project metadata at { $path } is missing the required field { $field }.
error-metadata-invalid-value = Field { $field } in { $path } is not valid: { $detail }
error-metadata-path-mismatch = The metadata at { $path } declares { $canonical_id }, which belongs at { $expected }.
error-metadata-duplicate-project = { $canonical_id } is declared by more than one project directory: { $paths }
error-sandbox-name-collision = Sandbox name { $sandbox } is derived from more than one project: { $projects }
error-invalid-branch-name = { $value } is not a usable branch name: { $detail }

error-project-path-unexpected-type = { $path } is a { $observed }, but sbxm expects a { $expected } there.
error-project-path-unreadable = The project path { $path } could not be read: { $detail }

error-atomic-write-failed = Writing { $path } failed: { $detail }
error-temp-file-left-behind = An interrupted run left the temporary file { $path } behind.
error-target-appeared-concurrently = { $path } appeared while it was being created, so nothing was overwritten.
error-target-changed-concurrently = { $path } was replaced by another file while it was being rewritten, so nothing was overwritten.
error-lock-timeout = Another sbxm run is holding the lock at { $path }. Waited { $seconds } seconds.
error-lock-unavailable = The lock at { $path } could not be acquired: { $detail }

error-external-command-not-found = The command { $program } was not found on PATH.
error-external-command-spawn-failed = The command { $program } could not be started: { $detail }
error-external-command-failed = The command { $program } failed with { $exit_status }.
error-external-command-timeout = The command { $program } did not finish within { $seconds } seconds and was terminated.
error-external-output-unparseable = The output of { $program } could not be interpreted: { $detail }
warning-external-output-lossy = The { $stream } output of { $program } was not valid UTF-8 and was converted with replacement characters.
external-output-heading = Output of { $program }:

error-sbx-version-unparseable = The Docker Sandboxes CLI version could not be determined from { $observed }.
error-sbx-version-below-minimum = Docker Sandboxes CLI { $observed } is older than the required { $minimum }.
error-platform-unsupported = This build supports { $expected }. Observed: { $observed }
error-platform-unobservable = The platform could not be determined: { $detail }
error-host-command-missing = The command { $command } was not found on PATH.
error-docker-unreachable = The Docker daemon did not answer: { $detail }
error-network-policy-mismatch = The network policy is { $observed }, but this build is validated only for { $expected }.
error-network-policy-unobservable = The network policy could not be read: { $detail }
error-daemon-unobservable = The Docker Sandboxes daemon state could not be read: { $detail }
remediation-run-help = Run { $command } to see the accepted arguments.
remediation-run-init = Run sbxm init to create the global configuration.
remediation-remove-temp-file = Inspect { $path }, then delete it once you are sure no other run is using it.
remediation-fix-config = Edit { $path } and run the command again.
remediation-install-command = Install { $command } and make sure it is on PATH.
remediation-start-docker = Start Docker Desktop and wait until the engine reports it is running.
remediation-network-policy = Set the Docker Sandboxes network policy to { $expected } before continuing.
remediation-wait-for-lock = Wait for the other sbxm run to finish, then run the command again.
security-config-permission-description = { $path } has mode { $observed }, which grants access beyond the owner. sbxm refuses to use a configuration that other accounts on this machine can read or change.
security-config-permission-remediation = Run chmod { $expected } { $path } and confirm that you own the file.

security-config-symlink-description = { $path } is a symbolic link. Following it could read or overwrite a file outside the sbxm configuration directory.
security-config-symlink-remediation = Replace { $path } with a regular file that you own, or move the real configuration back to that path.

security-config-dir-permission-description = { $path } has mode { $observed }, which grants access beyond the owner. Locks and configuration stored there could be observed or replaced.
security-config-dir-permission-remediation = Run chmod { $expected } { $path } and confirm that you own the directory.

security-config-dir-symlink-description = { $path } is a symbolic link. sbxm would create locks and configuration outside the intended directory.
security-config-dir-symlink-remediation = Replace { $path } with a directory that you own.

security-project-path-symlink-description = { $path } is a symbolic link. sbxm does not follow it, because creating or replacing files through it would act on a location outside the project directory.
security-project-path-symlink-remediation = Replace { $path } with a regular file or directory that you own, then run the command again.

security-project-file-permission-description = { $path } has mode { $observed }, which grants access beyond the owner. sbxm refuses to rewrite a file that other accounts on this machine can read or change.
security-project-file-permission-remediation = Run chmod { $expected } { $path } and confirm that you own the file.

security-base-path-escape-description = { $path } resolves to { $resolved } after symbolic links are followed. Projects would be created outside the directory you chose.
security-base-path-escape-remediation = Choose a base path whose resolved location stays inside the directory you intend to use.

init-already-initialized = sbxm is already initialized. The configuration is at { $path }.
init-created = The global configuration was created at { $path }.
init-next-step = Run sbxm status --global to diagnose the host environment.
init-prompt-language = Display language
init-prompt-base-path = Absolute path of the directory that will hold project directories
init-prompt-git-user-name = Git user.name to use inside sandboxes
init-prompt-git-user-email = Git user.email to use inside sandboxes
init-prompt-create-base-path = { $path } does not exist yet. Create it?
error-init-requires-tty = Interactive initialization needs both standard input and standard error to be a terminal.
error-git-identity-invalid = The Git { $field } value is not usable: { $detail }
detail-value-empty = the value is empty
detail-value-has-newline = the value contains a line break

status-global-section = GLOBAL
status-column-item = ITEM
status-column-status = STATUS
status-item-config = Config
status-item-base-path = Base path
status-item-platform = Platform
status-item-git = Git
status-item-ssh = SSH
status-item-docker = Docker
status-item-docker-sandboxes = Docker Sandboxes
status-item-network-policy = Network policy
status-item-daemon = Daemon
legend-heading = Legend
legend-ready = available and matching the expected state
legend-missing = not present
legend-error = could not be verified, or does not meet the requirement
legend-running = the service is running
legend-stopped = the service is installed but not running

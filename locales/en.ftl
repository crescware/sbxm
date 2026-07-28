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
error-sandbox-name-duplicated = The sandbox listing holds more than one sandbox named { $sandbox }, so no project can be paired with it.
error-invalid-branch-name = { $value } is not a usable branch name: { $detail }
error-target-configuration-mismatch = { $project } was registered to be built as { $stored }, but this run asks for { $requested }.
error-rebuild-intent-pending = { $project } is in the middle of a rebuild, so its first build cannot be continued.

error-host-clone-unusable = The clone at { $path } cannot be used for this project: { $detail }
error-image-unusable = The image { $image } cannot be used for this project: { $detail }
error-build-context-not-empty = The build context { $path } holds { $observed } entries, but sbxm builds only from an empty one.
warning-build-context-left-behind = The temporary build context { $path } could not be removed: { $detail }
error-archive-unusable = The template archive { $path } cannot be used: { $detail }
error-template-unusable = The template { $template } cannot be used: { $detail }
error-sandbox-unusable = The sandbox { $sandbox } cannot be used for this project: { $detail }
error-declared-file-unusable = The declared file { $source } cannot be placed: { $detail }
error-declared-file-conflict = { $destination } already holds different content, so { $source } was not placed.
error-sandbox-identity-mismatch = { $sandbox } already sets { $key } to { $observed }, and this project expects { $expected }.
error-github-secret-missing = The sandbox { $sandbox } has no { $secret } secret, so it cannot reach the repository.
error-sandbox-repository-unusable = { $path } in the sandbox cannot be used for this project: { $detail }
error-start-ref-unresolved = { $reference } does not exist on the remote of { $project }.
error-no-managed-projects = There is no managed project to choose from.
error-selection-unresolved = The selection { $index } does not name one of the { $count } candidates.
error-sandbox-still-running = The sandbox { $sandbox } was still running after it was asked to stop.
error-unsaved-work = { $target } holds work that would be lost: { $detail }
error-worktree-outside-repository = The worktree { $path } is outside the shared repository { $root }, so it is not an artifact of this project.
error-managed-worktree-missing = The project declares the managed worktree { $path }, but Git inside the sandbox does not list it.
error-worktrees-not-observed = No worktree was found under { $root }, so what the sandbox holds cannot be judged.
error-unmanaged-worktree-present = The worktree { $path } is not part of the target configuration, and a rebuild cannot recreate where it came from.
error-sandbox-still-present = The sandbox { $sandbox } was still listed after it was removed.
error-rebuild-generation-missing = The rebuild of { $project } is fixed on generation { $target }, and neither its artifacts nor a matching Dockerfile is present. The Dockerfile now holds { $observed }.
error-destroy-not-confirmed = The sandbox name was not entered exactly as { $sandbox }, so nothing was deleted.
error-sandbox-check-unobservable = The check for { $subject } inside the sandbox ended with { $exit_status } without answering, so its result is unknown.
error-global-scope-unobservable = Part of this project could not be inspected because the host environment could not be read.
error-project-not-managed = { $project } is not a managed project.
error-sandbox-not-created = { $project } is registered, but its sandbox { $sandbox } does not exist yet.
error-sandbox-not-running = The sandbox { $sandbox } is { $observed }, and this command only acts on a running sandbox.
warning-dockerfile-changed-during-rebuild = The Dockerfile of { $project } changed while the rebuild was already fixed on a generation, so this run applied the fixed one. Run { $command } again to apply the current Dockerfile.
warning-lock-file-left-behind = The project is no longer managed, but its lock file { $path } could not be removed: { $detail }
warning-dockerfile-changed-during-build = The Dockerfile of { $project } changed while its first build was still running, so the build finished with the generation it started from. Run { $command } to apply the current one.
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
external-invocation = Command: { $program } { $args }
external-working-directory = Working directory: { $path }
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
error-sbx-login-missing = This host is not signed in to Docker Sandboxes.
error-sbx-login-unobservable = Whether this host is signed in to Docker Sandboxes could not be read: { $detail }
error-remote-ssh-unconfigured = ssh has no proxy configuration for { $host }, so sbxm cannot reach a sandbox over SSH.
error-remote-ssh-unobservable = The SSH configuration for sandboxes could not be read: { $detail }
error-daemon-session-active = A session is connected to { $sandboxes }, so the Docker Sandboxes daemon was left as it is.
error-daemon-session-unobservable = This Docker Sandboxes version does not report whether a session is connected to { $sandbox }, so the daemon was left as it is.
remediation-run-help = Run { $command } to see the accepted arguments.
remediation-run-init = Run sbxm init to create the global configuration.
remediation-host-clone-unusable = Inspect { $path } yourself, then move it aside or fix its origin before running the command again.
remediation-daemon-session-active = Close the shells and editors connected to those sandboxes, then run the command again.
remediation-daemon-session-unobservable = Update the Docker Sandboxes CLI to a version that reports connected sessions, then run the command again.
remediation-declared-file-conflict = Compare the two yourself, then run sbxm sync-files to replace the file in the sandbox with the declared one.
remediation-sandbox-identity-mismatch = Check whose sandbox this is. sbxm does not overwrite a value that is already set.
remediation-github-secret-missing = Issue a fine-grained personal access token limited to this repository, with Contents read and write and Metadata read, plus Pull requests, Issues or Actions only if you need them. Register it with { $command }, then run the same command again.
remediation-sandbox-repository-unusable = Inspect { $path } inside the sandbox yourself. sbxm never deletes a repository or a worktree to make room.
remediation-start-ref-unresolved = Check the branch name on GitHub, then run the command again once the branch exists.
remediation-project-not-managed = Run { $command } to register and build it.
remediation-sandbox-not-created = Run { $command } to build the sandbox.
remediation-sandbox-not-running = Run { $command } to start the sandbox, then run this command again.
remediation-sbx-login = Run { $command } and finish signing in, then run this command again.
remediation-no-managed-projects = Run { $command } to register the first one.
remediation-diagnose-project = Run { $command } to see the state it is in.
remediation-run-global-status = Run { $command } to diagnose the host environment.
remediation-remote-ssh-unconfigured = Set up the Remote SSH integration of Docker Sandboxes on this host, then run this command again.
remediation-unsaved-work = Commit and push what you want to keep, remove what you do not, then run the command again.
remediation-worktree-outside-repository = Inspect that path yourself. sbxm never deletes a worktree it cannot account for.
remediation-managed-worktree-missing = Run { $command } to see what the sandbox actually holds, and settle the difference before deleting anything.
remediation-worktrees-not-observed = Run { $command } to see what the sandbox actually holds. sbxm does not act on a repository it cannot read.
remediation-unmanaged-worktree-present = Remove that worktree with git inside the sandbox once its work is saved, then run the rebuild again.
remediation-rebuild-generation-missing = Restore the Dockerfile of that generation and run the rebuild again. If it cannot be restored, { $command } deletes the sandbox and the management data, keeping the host clone and the Dockerfile.
remediation-destroy-force = Run { $command } to delete it without looking inside. Data protection and active session checks are skipped.
remediation-run-rebuild = Run { $command } to finish the rebuild.
remediation-target-configuration-mismatch = Run { $command } without those options to continue with the stored target, or destroy the project first to build it differently.
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

security-config-owner-description = { $path } belongs to user ID { $observed }, and you are user ID { $expected }. A configuration another account owns can be replaced under sbxm at any moment.
security-config-owner-remediation = Move { $path } out of the way and let sbxm create it again, or restore a file you own at that path.

security-config-dir-permission-description = { $path } has mode { $observed }, which grants access beyond the owner. Locks and configuration stored there could be observed or replaced.
security-config-dir-permission-remediation = Run chmod { $expected } { $path } and confirm that you own the directory.

security-config-dir-symlink-description = { $path } is a symbolic link. sbxm would create locks and configuration outside the intended directory.
security-config-dir-symlink-remediation = Replace { $path } with a directory that you own.

security-config-dir-owner-description = { $path } belongs to user ID { $observed }, and you are user ID { $expected }. Locks and configuration placed there would sit in a directory another account controls.
security-config-dir-owner-remediation = Move { $path } out of the way and let sbxm create it again, or restore a directory you own at that path.

security-project-path-symlink-description = { $path } is a symbolic link. sbxm does not follow it, because creating or replacing files through it would act on a location outside the project directory.
security-project-path-symlink-remediation = Replace { $path } with a regular file or directory that you own, then run the command again.

security-project-file-permission-description = { $path } has mode { $observed }, which grants access beyond the owner. sbxm refuses to use a file that other accounts on this machine can read or change.
security-project-file-permission-remediation = Run chmod { $expected } { $path } and confirm that you own the file.

security-project-path-owner-description = { $path } belongs to user ID { $observed }, and you are user ID { $expected }. sbxm does not build a project on a path another account owns, whatever its mode says.
security-project-path-owner-remediation = Move { $path } out of the way and let sbxm create it again, or restore a path you own there.

security-ssh-agent-exposed-description = The host SSH agent can be reached from { $sandbox }. An agent inside the sandbox can sign with your keys.
security-ssh-agent-exposed-remediation = Stop the sandbox, then open it again with sbxm so that the daemon is restarted without the agent.

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
add-field-project = Project
add-field-sandbox = Sandbox
add-field-creation-mode = Creation mode
add-field-start-branch = Start branch
add-field-managed-worktrees = Managed worktree count
add-field-host-clone = Host clone
add-field-sandbox-state = Sandbox state
ls-projects-section = PROJECTS
ls-unmanaged-section = UNMANAGED SANDBOXES
column-project = PROJECT
column-sandbox = SANDBOX
column-state = STATE
column-workspace = WORKSPACE
column-worktree = WORKTREE
column-created-from = CREATED FROM
column-head = HEAD
column-mode = MODE
column-file = FILE
column-destination = DESTINATION
column-result = RESULT
add-already-built = { $project } is already built, so nothing was changed.
add-mise-heading = These managed worktrees carry a mise configuration. sbxm does not run mise for you:
add-mise-hint = Run mise trust and mise install inside the sandbox when you want to use them.
files-secret-hint = Declared files carry configuration, not credentials. Keep tokens, secrets and private keys out of them and hand those to the sandbox with the secret feature of Docker Sandboxes instead.
sync-files-done = { $count } declared files of { $project } were placed into { $sandbox }.
legend-attached = the worktree follows a branch that tracks the remote
legend-detached = the worktree sits on a commit without a branch
legend-placed = the file was written into the sandbox
legend-unchanged = the sandbox already held the same content

status-item-login = Docker Sandboxes login
status-item-session-inspection = Active session inspection
open-connecting = Connecting to { $sandbox } for { $project }.
open-worktrees = Managed worktrees in this sandbox:

destroy-confirm-prompt = Type the sandbox name to confirm the deletion
destroy-removes = These are deleted:
destroy-keeps = These are kept:
destroy-force-notice = Force mode skips the data protection and active session checks.
destroy-done = { $project } is no longer managed.
destroy-re-register = Register it again with: { $command }

column-branch = BRANCH
column-remote = REMOTE
select-destroy-heading = Which project do you want to destroy?
rebuild-unchanged = The Dockerfile of { $project } is the one that is applied, so nothing was changed.
rebuild-applied = { $project } was rebuilt: { $sandbox } now runs generation { $generation }.

select-open-heading = Which project do you want to open?
select-stop-heading = Which projects do you want to stop?

legend-stopped-now = the sandbox was stopped by this run
legend-not-stopped = not stopped by this run: it was already stopped, has no sandbox, or was left alone after an earlier failure
legend-failed = the sandbox could not be stopped
status-project-section = PROJECT
status-worktrees-section = WORKTREES
status-column-value = VALUE
status-item-metadata = Metadata
status-item-project-root = Project root
status-item-host-clone = Host clone
status-item-dockerfile = Dockerfile
status-item-image = Image
status-item-template-archive = Template archive
status-item-sandbox = Sandbox
status-item-workspace = Workspace
status-item-secret = GitHub secret
status-item-bare-repository = Bare repository
status-item-ssh-agent = SSH Agent
status-item-worktrees = Worktrees
status-item-project = Project
column-path = PATH
column-kind = KIND
legend-mismatch = the observed state does not match what the project declares
legend-changed = the file changed since it was applied, so the next rebuild will pick it up
legend-clean = the working tree has no change to commit
legend-dirty = the working tree has changes
legend-not-exposed = the host SSH agent cannot be reached from the sandbox
legend-exposed = the host SSH agent can be reached from the sandbox
legend-not-applicable = there is no sandbox to look into
legend-not-observed-stopped = the sandbox is stopped, and sbxm did not start it to look
status-item-remote-ssh = Remote SSH
legend-heading = Legend
legend-ready = available and matching the expected state
legend-missing = not present
legend-error = could not be verified, or does not meet the requirement
legend-not-created = the project is registered, but its sandbox does not exist yet
legend-sandbox-running = the sandbox is running
legend-sandbox-stopped = the sandbox exists but is not running
legend-running = the service is running
legend-stopped = the service is installed but not running

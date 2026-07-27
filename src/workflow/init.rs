//! `sbxm init`。
//!
//! configがない場合だけ新規作成する。既存の有効configは再利用し、無効configは停止する。
//! host環境の診断、login、setupは行わない。

use std::path::{Path, PathBuf};

use crate::cli::{InitMode, Interactivity};
use crate::command::{CommandSpec, TimeoutClass};
use crate::config::{
    self, ConfigLocation, ConfigState, GitIdentity, GlobalConfig, validate_git_identity_value,
};
use crate::error::{Error, ErrorId, Msg, Result, fail};
use crate::i18n::{Catalog, Locale, shell_locale};
use crate::msg;
use crate::paths::{self, AbsoluteBasePath, LOCK_TIMEOUT, PRIVATE_FILE_MODE};

use super::status_global::HostEnvironment;

/// 対話modeの入力。
///
/// promptはstdinから読み、stderrへ表示する。
pub trait Prompt {
    fn select_language(&mut self, catalog: &Catalog) -> Result<Locale>;
    fn base_path(&mut self, catalog: &Catalog) -> Result<String>;
    fn git_user_name(&mut self, catalog: &Catalog, candidate: &str) -> Result<String>;
    fn git_user_email(&mut self, catalog: &Catalog, candidate: &str) -> Result<String>;
    fn confirm_create_base_path(&mut self, catalog: &Catalog, path: &Path) -> Result<bool>;
}

/// `init`の入力。
pub struct InitRequest {
    pub mode: InitMode,
    /// 有効な`--lang`。mode判定には含めない。
    pub lang: Option<Locale>,
    pub interactivity: Interactivity,
}

/// `init`の結果。
#[derive(Debug)]
pub struct InitOutput {
    /// この実行で確定した表示言語。
    pub locale: Locale,
    /// 既に初期化済みで、何も変更しなかったか。
    pub already_initialized: bool,
    pub config_path: PathBuf,
    pub warnings: Vec<Msg>,
}

/// `sbxm init`を実行する。
pub fn run(
    location: &ConfigLocation,
    request: &InitRequest,
    host: &dyn HostEnvironment,
    prompt: &mut dyn Prompt,
) -> Result<InitOutput> {
    // 3. configをread-onlyで事前確認する。
    // 5. 無効なconfigはここで伝播し、自動修復も上書きもしない。
    if let Some(output) = already_initialized(location, request)? {
        return Ok(output);
    }

    // 6. 新規作成へ進む対話modeはstdinとstderrの両方がTTYであることを必須とする。
    if matches!(request.mode, InitMode::Interactive) && !request.interactivity.can_prompt() {
        return fail(ErrorId::InitRequiresTty, msg!("error-init-requires-tty"));
    }

    // 7. `~/.sbxm`を検証または作成し、init.lockを取得する。
    config::ensure_config_dir(location)?;
    let lock_path = location.init_lock();
    let _lock = paths::acquire_exclusive_lock(&lock_path, LOCK_TIMEOUT, PRIVATE_FILE_MODE)?;

    // 8-9. lock取得後にconfigの有無と妥当性を再確認する。
    if let Some(output) = already_initialized(location, request)? {
        return Ok(output);
    }

    let locale = resolve_locale(request, host, prompt)?;
    let catalog = Catalog::new(locale);

    let (base_path, git) = match &request.mode {
        InitMode::Options {
            base_path,
            git_user_name,
            git_user_email,
        } => {
            // 11. option modeでは完全指定された値をpromptなしで検証する。
            let base_path = AbsoluteBasePath::new(Path::new(base_path))?;
            let git = GitIdentity {
                user_name: check_identity(&catalog, "user.name", git_user_name)?,
                user_email: check_identity(&catalog, "user.email", git_user_email)?,
            };
            ensure_base_path_exists(&base_path, &catalog, prompt, false)?;
            (base_path, git)
        }
        InitMode::Interactive => {
            // 10. 対話modeでは、base path、Git name、Git emailをpromptで取得・検証する。
            let declared = prompt.base_path(&catalog)?;
            let base_path = AbsoluteBasePath::new(Path::new(declared.trim()))?;
            ensure_base_path_exists(&base_path, &catalog, prompt, true)?;

            let git = GitIdentity {
                user_name: check_identity(
                    &catalog,
                    "user.name",
                    &prompt.git_user_name(&catalog, &git_candidate(host, "user.name"))?,
                )?,
                user_email: check_identity(
                    &catalog,
                    "user.email",
                    &prompt.git_user_email(&catalog, &git_candidate(host, "user.email"))?,
                )?,
            };
            (base_path, git)
        }
    };

    // 12. configをatomic writeする。
    let config = GlobalConfig {
        language: locale,
        base_path,
        git,
        files: Vec::new(),
    };
    let config_path = config::create(location, &config)?;

    Ok(InitOutput {
        locale,
        already_initialized: false,
        config_path,
        warnings: Vec::new(),
    })
}

/// 既に有効なconfigがあれば、何も変更せず終了するための結果を返す。
fn already_initialized(
    location: &ConfigLocation,
    request: &InitRequest,
) -> Result<Option<InitOutput>> {
    match config::load(location)? {
        ConfigState::Valid { config, warnings } => Ok(Some(InitOutput {
            locale: request.lang.unwrap_or(config.language),
            already_initialized: true,
            config_path: location.config_file(),
            warnings,
        })),
        ConfigState::Missing => Ok(None),
    }
}

/// Git identityの値を検証する。
///
/// 不正の理由もFTLから生成するため、確定済みのcatalogで解決してから埋め込む。
fn check_identity(catalog: &Catalog, field: &str, value: &str) -> Result<String> {
    match validate_git_identity_value(value) {
        Ok(()) => Ok(value.to_string()),
        Err(detail_id) => {
            let detail = catalog
                .text(detail_id)
                .unwrap_or_else(|failure| failure.to_string());
            fail(
                ErrorId::GitIdentityInvalid,
                msg!("error-git-identity-invalid", field = field, detail = detail),
            )
        }
    }
}

/// base pathが存在しなければ、確認のうえ作成する。
fn ensure_base_path_exists(
    base_path: &AbsoluteBasePath,
    catalog: &Catalog,
    prompt: &mut dyn Prompt,
    confirm: bool,
) -> Result<()> {
    let path = base_path.as_path();
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => fail(
            ErrorId::BasePathNotDirectory,
            msg!("error-base-path-not-directory", path = paths::display(path)),
        ),
        Err(_) => {
            if confirm && !prompt.confirm_create_base_path(catalog, path)? {
                return Err(Error::Canceled);
            }
            std::fs::create_dir_all(path).map_err(|error| {
                Error::new(
                    ErrorId::AtomicWriteFailed,
                    msg!(
                        "error-atomic-write-failed",
                        path = paths::display(path),
                        detail = error
                    ),
                )
            })
        }
    }
}

/// `init`実行時のlocale決定。
///
/// 1. 有効な`--lang`
/// 2. macOS優先言語
/// 3. shell locale
/// 4. 正本locale
fn resolve_locale(
    request: &InitRequest,
    host: &dyn HostEnvironment,
    prompt: &mut dyn Prompt,
) -> Result<Locale> {
    if let Some(locale) = request.lang {
        return Ok(locale);
    }

    let preferred = macos_preferred_language(host);
    match (&request.mode, preferred) {
        // 対話modeで正本locale以外が推測された場合だけ、その推測を確認させる。
        (InitMode::Interactive, Some(locale)) if !locale.is_source() => {
            let bootstrap = Catalog::new(locale);
            prompt.select_language(&bootstrap)
        }
        // option modeではpromptを表示しない。
        (_, Some(locale)) => Ok(locale),
        (_, None) => Ok(shell_locale().unwrap_or(Locale::SOURCE)),
    }
}

/// macOS優先言語の先頭要素。
///
/// command失敗またはparse失敗時だけshell localeへfallbackする。
fn macos_preferred_language(host: &dyn HostEnvironment) -> Option<Locale> {
    let spec = CommandSpec::probe("defaults", &["read", "-g", "AppleLanguages"])
        .timeout(TimeoutClass::Probe);
    let outcome = host.run(&spec).ok()?;
    if !outcome.success() {
        return None;
    }
    parse_apple_languages(&outcome.stdout_text())
}

/// `defaults read -g AppleLanguages`の出力から先頭のlanguage tagを取り出す。
fn parse_apple_languages(output: &str) -> Option<Locale> {
    let start = output.find('"')?;
    let rest = &output[start + 1..];
    let end = rest.find('"')?;
    Locale::from_language_tag(&rest[..end])
}

/// hostのGit identityを既定候補として読む。取得できない場合は空文字とする。
fn git_candidate(host: &dyn HostEnvironment, key: &str) -> String {
    let spec = CommandSpec::probe("git", &["config", "--global", key])
        .timeout(TimeoutClass::LocalFilesystem);
    match host.run(&spec) {
        Ok(outcome) if outcome.success() => outcome.stdout_text().trim().to_string(),
        _ => String::new(),
    }
}

/// dialoguerを使う対話実装。
pub struct TerminalPrompt;

impl TerminalPrompt {
    fn text(catalog: &Catalog, id: &str) -> String {
        catalog
            .text(id)
            .unwrap_or_else(|failure| failure.to_string())
    }

    /// EscとCtrl-Cは何も変更せずexit code `130`とする。
    fn map_error(error: dialoguer::Error) -> Error {
        match error {
            dialoguer::Error::IO(io) if io.kind() == std::io::ErrorKind::Interrupted => {
                Error::Canceled
            }
            _ => Error::new(ErrorId::InitRequiresTty, msg!("error-init-requires-tty")),
        }
    }
}

impl Prompt for TerminalPrompt {
    fn select_language(&mut self, catalog: &Catalog) -> Result<Locale> {
        // 推測されたlocaleを先頭に、残りを定義順で並べる。
        let guessed = catalog.locale();
        let mut choices = vec![guessed];
        choices.extend(Locale::ALL.into_iter().filter(|locale| *locale != guessed));

        // 言語の名称は、その言語自身のresourceが持つ自称表記を使う。
        let items: Vec<String> = choices
            .iter()
            .map(|locale| TerminalPrompt::text(&Catalog::new(*locale), "locale-name"))
            .collect();

        let index = dialoguer::Select::new()
            .with_prompt(TerminalPrompt::text(catalog, "init-prompt-language"))
            .items(&items)
            .interact()
            .map_err(TerminalPrompt::map_error)?;
        Ok(*choices
            .get(index)
            .expect("the selection index stays within the offered items"))
    }

    fn base_path(&mut self, catalog: &Catalog) -> Result<String> {
        dialoguer::Input::<String>::new()
            .with_prompt(TerminalPrompt::text(catalog, "init-prompt-base-path"))
            .interact_text()
            .map_err(TerminalPrompt::map_error)
    }

    fn git_user_name(&mut self, catalog: &Catalog, candidate: &str) -> Result<String> {
        let mut input = dialoguer::Input::<String>::new()
            .with_prompt(TerminalPrompt::text(catalog, "init-prompt-git-user-name"));
        if !candidate.is_empty() {
            // 候補を表示して明示確定させる。
            input = input.with_initial_text(candidate);
        }
        input.interact_text().map_err(TerminalPrompt::map_error)
    }

    fn git_user_email(&mut self, catalog: &Catalog, candidate: &str) -> Result<String> {
        let mut input = dialoguer::Input::<String>::new()
            .with_prompt(TerminalPrompt::text(catalog, "init-prompt-git-user-email"));
        if !candidate.is_empty() {
            input = input.with_initial_text(candidate);
        }
        input.interact_text().map_err(TerminalPrompt::map_error)
    }

    fn confirm_create_base_path(&mut self, catalog: &Catalog, path: &Path) -> Result<bool> {
        let message = catalog
            .format(&msg!(
                "init-prompt-create-base-path",
                path = paths::display(path)
            ))
            .unwrap_or_else(|failure| failure.to_string());
        dialoguer::Confirm::new()
            .with_prompt(message)
            .interact()
            .map_err(TerminalPrompt::map_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandOutcome;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;

    struct FakeHost {
        responses: HashMap<String, String>,
    }

    impl FakeHost {
        fn new() -> FakeHost {
            FakeHost {
                responses: HashMap::new(),
            }
        }

        fn responding(mut self, key: &str, stdout: &str) -> FakeHost {
            self.responses.insert(key.to_string(), stdout.to_string());
            self
        }
    }

    impl HostEnvironment for FakeHost {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
            let key = format!("{} {}", spec.program, spec.args.join(" "));
            match self.responses.get(&key) {
                Some(stdout) => Ok(CommandOutcome {
                    program: spec.program.clone(),
                    args: spec.args.clone(),
                    cwd: None,
                    status: std::process::ExitStatus::from_raw(0),
                    stdout: stdout.clone().into_bytes(),
                    stderr: Vec::new(),
                    stdout_lossy: false,
                    stderr_lossy: false,
                }),
                None => Err(Error::new(
                    ErrorId::ExternalCommandNotFound,
                    msg!("error-external-command-not-found", program = spec.program),
                )),
            }
        }
    }

    #[derive(Default)]
    struct ScriptedPrompt {
        language: Option<Locale>,
        base_path: Option<String>,
        user_name: Option<String>,
        user_email: Option<String>,
        create_base_path: bool,
        canceled: bool,
        calls: RefCell<Vec<&'static str>>,
        candidates: RefCell<Vec<String>>,
    }

    impl ScriptedPrompt {
        fn answering(base_path: &Path, name: &str, email: &str) -> ScriptedPrompt {
            ScriptedPrompt {
                base_path: Some(base_path.display().to_string()),
                user_name: Some(name.to_string()),
                user_email: Some(email.to_string()),
                create_base_path: true,
                ..ScriptedPrompt::default()
            }
        }

        fn record(&self, call: &'static str) {
            self.calls.borrow_mut().push(call);
        }
    }

    impl Prompt for ScriptedPrompt {
        fn select_language(&mut self, _catalog: &Catalog) -> Result<Locale> {
            self.record("select_language");
            if self.canceled {
                return Err(Error::Canceled);
            }
            Ok(self.language.unwrap_or(Locale::En))
        }

        fn base_path(&mut self, _catalog: &Catalog) -> Result<String> {
            self.record("base_path");
            if self.canceled {
                return Err(Error::Canceled);
            }
            Ok(self.base_path.clone().unwrap_or_default())
        }

        fn git_user_name(&mut self, _catalog: &Catalog, candidate: &str) -> Result<String> {
            self.record("git_user_name");
            self.candidates.borrow_mut().push(candidate.to_string());
            Ok(self.user_name.clone().unwrap_or_default())
        }

        fn git_user_email(&mut self, _catalog: &Catalog, candidate: &str) -> Result<String> {
            self.record("git_user_email");
            self.candidates.borrow_mut().push(candidate.to_string());
            Ok(self.user_email.clone().unwrap_or_default())
        }

        fn confirm_create_base_path(&mut self, _catalog: &Catalog, _path: &Path) -> Result<bool> {
            self.record("confirm_create_base_path");
            Ok(self.create_base_path)
        }
    }

    fn home() -> (tempfile::TempDir, ConfigLocation) {
        let dir = tempfile::tempdir().expect("temporary home");
        let location = ConfigLocation::from_home(dir.path().to_path_buf());
        (dir, location)
    }

    fn tty() -> Interactivity {
        Interactivity {
            stdin_is_tty: true,
            stderr_is_tty: true,
        }
    }

    fn non_tty() -> Interactivity {
        Interactivity {
            stdin_is_tty: false,
            stderr_is_tty: false,
        }
    }

    fn option_mode(base: &Path) -> InitMode {
        InitMode::Options {
            base_path: base.display().to_string(),
            git_user_name: "Example User".into(),
            git_user_email: "user@example.com".into(),
        }
    }

    #[test]
    fn option_mode_creates_the_configuration_without_prompting() {
        let (dir, location) = home();
        let base = dir.path().join("Projects");
        let mut prompt = ScriptedPrompt::default();

        let output = run(
            &location,
            &InitRequest {
                mode: option_mode(&base),
                lang: Some(Locale::En),
                interactivity: non_tty(),
            },
            &FakeHost::new(),
            &mut prompt,
        )
        .expect("option mode runs without a terminal");

        assert!(!output.already_initialized);
        assert_eq!(output.locale, Locale::En);
        assert!(output.config_path.exists());
        assert!(
            prompt.calls.borrow().is_empty(),
            "option mode must not prompt: {:?}",
            prompt.calls.borrow()
        );
        assert!(base.is_dir(), "the base path is created");
    }

    #[test]
    fn interactive_mode_outside_a_terminal_creates_nothing() {
        let (dir, location) = home();
        let mut prompt = ScriptedPrompt::default();

        let error = run(
            &location,
            &InitRequest {
                mode: InitMode::Interactive,
                lang: None,
                interactivity: non_tty(),
            },
            &FakeHost::new(),
            &mut prompt,
        )
        .expect_err("interactive initialization needs a terminal");

        assert_eq!(error.first_id(), Some(ErrorId::InitRequiresTty));
        assert!(!location.dir().exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn interactive_mode_asks_for_every_value_and_offers_the_host_git_identity() {
        let (dir, location) = home();
        let base = dir.path().join("Projects");
        let mut prompt = ScriptedPrompt::answering(&base, "Prompted User", "prompted@example.com");
        let host = FakeHost::new()
            .responding("git config --global user.name", "Host User\n")
            .responding("git config --global user.email", "host@example.com\n");

        let output = run(
            &location,
            &InitRequest {
                mode: InitMode::Interactive,
                lang: Some(Locale::Ja),
                interactivity: tty(),
            },
            &host,
            &mut prompt,
        )
        .expect("interactive mode completes");

        assert_eq!(output.locale, Locale::Ja);
        assert_eq!(
            *prompt.candidates.borrow(),
            vec!["Host User".to_string(), "host@example.com".to_string()],
            "the host Git identity is offered as the candidate"
        );
        let written = std::fs::read_to_string(&output.config_path).unwrap();
        assert!(written.contains("Prompted User"), "{written}");
        assert!(
            !written.contains("Host User"),
            "the candidate must be confirmed, not applied silently: {written}"
        );
    }

    #[test]
    fn declining_to_create_the_base_path_cancels_without_writing_anything() {
        let (dir, location) = home();
        let base = dir.path().join("Projects");
        let mut prompt = ScriptedPrompt {
            create_base_path: false,
            ..ScriptedPrompt::answering(&base, "Example User", "user@example.com")
        };

        let error = run(
            &location,
            &InitRequest {
                mode: InitMode::Interactive,
                lang: Some(Locale::En),
                interactivity: tty(),
            },
            &FakeHost::new(),
            &mut prompt,
        )
        .expect_err("declining the directory cancels the run");

        assert_eq!(error.exit_code(), crate::error::ExitCode::Canceled);
        assert!(!location.config_file().exists());
        assert!(!base.exists());
    }

    #[test]
    fn cancelling_a_prompt_exits_with_130_and_changes_nothing() {
        let (dir, location) = home();
        let mut prompt = ScriptedPrompt {
            canceled: true,
            ..ScriptedPrompt::answering(&dir.path().join("Projects"), "n", "e")
        };

        let error = run(
            &location,
            &InitRequest {
                mode: InitMode::Interactive,
                lang: Some(Locale::En),
                interactivity: tty(),
            },
            &FakeHost::new(),
            &mut prompt,
        )
        .expect_err("a cancelled prompt does not create a configuration");

        assert_eq!(error.exit_code(), crate::error::ExitCode::Canceled);
        assert!(!location.config_file().exists());
    }

    #[test]
    fn re_running_init_is_a_no_op_success_in_both_tty_modes() {
        let (dir, location) = home();
        let base = dir.path().join("Projects");
        let mut prompt = ScriptedPrompt::default();

        run(
            &location,
            &InitRequest {
                mode: option_mode(&base),
                lang: Some(Locale::Ja),
                interactivity: non_tty(),
            },
            &FakeHost::new(),
            &mut prompt,
        )
        .expect("first run creates the configuration");
        let first = std::fs::read_to_string(location.config_file()).unwrap();

        for interactivity in [tty(), non_tty()] {
            let output = run(
                &location,
                &InitRequest {
                    mode: InitMode::Interactive,
                    lang: None,
                    interactivity,
                },
                &FakeHost::new(),
                &mut prompt,
            )
            .expect("re-running init succeeds");

            assert!(output.already_initialized);
            assert_eq!(output.locale, Locale::Ja, "the stored language is reused");
            assert_eq!(
                std::fs::read_to_string(location.config_file()).unwrap(),
                first,
                "an initialized host must not be modified"
            );
        }
        assert!(prompt.calls.borrow().is_empty());
    }

    #[test]
    fn an_invalid_configuration_stops_init_instead_of_being_repaired() {
        let (_dir, location) = home();
        std::fs::create_dir_all(location.dir()).unwrap();
        std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(location.config_file(), "version = 99\n").unwrap();
        std::fs::set_permissions(
            location.config_file(),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();

        let mut prompt = ScriptedPrompt::default();
        let error = run(
            &location,
            &InitRequest {
                mode: InitMode::Interactive,
                lang: None,
                interactivity: tty(),
            },
            &FakeHost::new(),
            &mut prompt,
        )
        .expect_err("an invalid configuration is not repaired");

        assert_eq!(error.first_id(), Some(ErrorId::ConfigUnknownVersion));
        assert_eq!(
            std::fs::read_to_string(location.config_file()).unwrap(),
            "version = 99\n"
        );
    }

    #[test]
    fn the_lock_file_survives_and_the_configuration_directory_is_private() {
        let (dir, location) = home();
        let mut prompt = ScriptedPrompt::default();

        run(
            &location,
            &InitRequest {
                mode: option_mode(&dir.path().join("Projects")),
                lang: Some(Locale::En),
                interactivity: non_tty(),
            },
            &FakeHost::new(),
            &mut prompt,
        )
        .expect("init runs");

        assert!(
            location.init_lock().exists(),
            "init.lock is not deleted when the workflow ends"
        );
        let mode = std::fs::metadata(location.dir())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[test]
    fn a_concurrent_init_waits_and_then_observes_the_finished_configuration() {
        let (dir, location) = home();
        let base = dir.path().join("Projects");
        config::ensure_config_dir(&location).unwrap();

        // 先行processがlockを保持している状態を作る。
        let held =
            paths::acquire_exclusive_lock(&location.init_lock(), LOCK_TIMEOUT, PRIVATE_FILE_MODE)
                .unwrap();

        let location_for_thread = location.clone();
        let base_for_thread = base.clone();
        let waiter = std::thread::spawn(move || {
            let mut prompt = ScriptedPrompt::default();
            run(
                &location_for_thread,
                &InitRequest {
                    mode: InitMode::Options {
                        base_path: base_for_thread.display().to_string(),
                        git_user_name: "Second User".into(),
                        git_user_email: "second@example.com".into(),
                    },
                    lang: Some(Locale::En),
                    interactivity: non_tty(),
                },
                &FakeHost::new(),
                &mut prompt,
            )
        });

        // 先行processがconfigを作ってからlockを解放する。
        std::thread::sleep(std::time::Duration::from_millis(50));
        let config = GlobalConfig {
            language: Locale::Ja,
            base_path: AbsoluteBasePath::new(&base).unwrap(),
            git: GitIdentity {
                user_name: "First User".into(),
                user_email: "first@example.com".into(),
            },
            files: Vec::new(),
        };
        std::fs::create_dir_all(&base).unwrap();
        config::create(&location, &config).unwrap();
        drop(held);

        let output = waiter
            .join()
            .expect("the waiting thread finishes")
            .expect("the second run observes the finished configuration");

        assert!(
            output.already_initialized,
            "the second run must not create a second configuration"
        );
        let written = std::fs::read_to_string(location.config_file()).unwrap();
        assert!(written.contains("First User"), "{written}");
        assert!(!written.contains("Second User"), "{written}");
    }

    #[test]
    fn a_lock_held_for_longer_than_the_timeout_fails_without_writing() {
        let (dir, location) = home();
        config::ensure_config_dir(&location).unwrap();
        let _held =
            paths::acquire_exclusive_lock(&location.init_lock(), LOCK_TIMEOUT, PRIVATE_FILE_MODE)
                .unwrap();

        // timeoutの経過をtestで待たないよう、lock取得だけを直接検証する。
        let error = paths::acquire_exclusive_lock(
            &location.init_lock(),
            std::time::Duration::from_millis(100),
            PRIVATE_FILE_MODE,
        )
        .expect_err("a held lock is not stolen");
        assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));
        assert!(!location.config_file().exists());
        let _ = dir;
    }

    #[test]
    fn a_git_identity_that_cannot_be_used_is_refused() {
        let (dir, location) = home();
        let mut prompt = ScriptedPrompt::default();

        for (name, email) in [("", "user@example.com"), ("Example User", "a\nb")] {
            let error = run(
                &location,
                &InitRequest {
                    mode: InitMode::Options {
                        base_path: dir.path().join("Projects").display().to_string(),
                        git_user_name: name.into(),
                        git_user_email: email.into(),
                    },
                    lang: Some(Locale::En),
                    interactivity: non_tty(),
                },
                &FakeHost::new(),
                &mut prompt,
            )
            .expect_err("unusable identities are refused");
            assert_eq!(error.first_id(), Some(ErrorId::GitIdentityInvalid));
            assert!(!location.config_file().exists());
        }
    }

    #[test]
    fn a_relative_base_path_is_refused_before_anything_is_created() {
        let (_dir, location) = home();
        let mut prompt = ScriptedPrompt::default();

        let error = run(
            &location,
            &InitRequest {
                mode: InitMode::Options {
                    base_path: "relative/projects".into(),
                    git_user_name: "Example User".into(),
                    git_user_email: "user@example.com".into(),
                },
                lang: Some(Locale::En),
                interactivity: non_tty(),
            },
            &FakeHost::new(),
            &mut prompt,
        )
        .expect_err("relative base paths are refused");

        assert_eq!(error.first_id(), Some(ErrorId::BasePathNotAbsolute));
        assert!(!location.config_file().exists());
    }

    #[test]
    fn the_macos_preferred_language_is_read_only_when_lang_is_absent() {
        let host = FakeHost::new().responding(
            "defaults read -g AppleLanguages",
            "(\n    \"ja-JP\",\n    \"en-US\"\n)\n",
        );
        let mut prompt = ScriptedPrompt {
            language: Some(Locale::Ja),
            ..ScriptedPrompt::default()
        };

        // `--lang`があるとpromptもmacOS優先言語も参照しない。
        let locale = resolve_locale(
            &InitRequest {
                mode: InitMode::Interactive,
                lang: Some(Locale::En),
                interactivity: tty(),
            },
            &host,
            &mut prompt,
        )
        .unwrap();
        assert_eq!(locale, Locale::En);
        assert!(prompt.calls.borrow().is_empty());

        // 対話modeで先頭がjaなら選択させる。
        let locale = resolve_locale(
            &InitRequest {
                mode: InitMode::Interactive,
                lang: None,
                interactivity: tty(),
            },
            &host,
            &mut prompt,
        )
        .unwrap();
        assert_eq!(locale, Locale::Ja);
        assert_eq!(*prompt.calls.borrow(), vec!["select_language"]);
    }

    #[test]
    fn option_mode_never_prompts_for_the_language() {
        let host =
            FakeHost::new().responding("defaults read -g AppleLanguages", "(\n    \"ja-JP\"\n)\n");
        let mut prompt = ScriptedPrompt::default();
        let locale = resolve_locale(
            &InitRequest {
                mode: InitMode::Options {
                    base_path: "/tmp/projects".into(),
                    git_user_name: "n".into(),
                    git_user_email: "e".into(),
                },
                lang: None,
                interactivity: non_tty(),
            },
            &host,
            &mut prompt,
        )
        .unwrap();
        assert_eq!(locale, Locale::Ja);
        assert!(prompt.calls.borrow().is_empty());
    }

    #[test]
    fn apple_languages_output_is_reduced_to_the_first_entry() {
        assert_eq!(
            parse_apple_languages("(\n    \"ja-JP\",\n    \"en-US\"\n)\n"),
            Some(Locale::Ja)
        );
        assert_eq!(
            parse_apple_languages("(\n    \"en-US\",\n    \"ja-JP\"\n)\n"),
            Some(Locale::En)
        );
        assert_eq!(parse_apple_languages("(\n    \"zz-ZZ\"\n)\n"), None);
        assert_eq!(parse_apple_languages(""), None);
    }
}

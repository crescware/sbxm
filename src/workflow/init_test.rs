use super::*;
use crate::command::CommandOutcome;
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;

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
            Some(stdout) => Ok(crate::testing::command::outcome(spec, 0, stdout)),
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
    let held = paths::acquire_exclusive_lock(
        &location.init_lock(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
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
    let _held = paths::acquire_exclusive_lock(
        &location.init_lock(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .unwrap();

    // timeoutの経過をtestで待たないよう、lock取得だけを直接検証する。
    let error = paths::acquire_exclusive_lock(
        &location.init_lock(),
        std::time::Duration::from_millis(100),
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
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
fn a_guessed_source_locale_is_not_put_to_the_user() {
    let host =
        FakeHost::new().responding("defaults read -g AppleLanguages", "(\n    \"en-US\"\n)\n");
    let mut prompt = ScriptedPrompt {
        language: Some(Locale::Ja),
        ..ScriptedPrompt::default()
    };

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

    assert_eq!(locale, Locale::En);
    assert!(prompt.calls.borrow().is_empty());
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

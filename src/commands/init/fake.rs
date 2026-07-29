//! `init`のtestが使うhostとpromptのfake。
//!
//! `init`は対話とhostのgit設定から値を決めるため、その両方をtestが組み立てる。

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use crate::cli::Interactivity;
use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::config::ConfigLocation;
use crate::error::{Error, ErrorId, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;

use super::Mode;
use super::run::Prompt;

pub struct FakeHost {
    responses: HashMap<String, String>,
}

impl FakeHost {
    pub fn new() -> FakeHost {
        FakeHost {
            responses: HashMap::new(),
        }
    }

    pub fn responding(mut self, key: &str, stdout: &str) -> FakeHost {
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
pub struct ScriptedPrompt {
    pub language: Option<Locale>,
    pub base_path: Option<String>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub create_base_path: bool,
    pub canceled: bool,
    pub calls: RefCell<Vec<&'static str>>,
    pub candidates: RefCell<Vec<String>>,
}

impl ScriptedPrompt {
    pub fn answering(base_path: &Path, name: &str, email: &str) -> ScriptedPrompt {
        ScriptedPrompt {
            base_path: Some(base_path.display().to_string()),
            user_name: Some(name.to_string()),
            user_email: Some(email.to_string()),
            create_base_path: true,
            ..ScriptedPrompt::default()
        }
    }

    pub fn record(&self, call: &'static str) {
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

pub fn home() -> (tempfile::TempDir, ConfigLocation) {
    let dir = tempfile::tempdir().expect("temporary home");
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    (dir, location)
}

pub fn tty() -> Interactivity {
    Interactivity {
        stdin_is_tty: true,
        stderr_is_tty: true,
    }
}

pub fn non_tty() -> Interactivity {
    Interactivity {
        stdin_is_tty: false,
        stderr_is_tty: false,
    }
}

pub fn option_mode(base: &Path) -> Mode {
    Mode::Options {
        base_path: base.display().to_string(),
        git_user_name: "Example User".into(),
        git_user_email: "user@example.com".into(),
    }
}

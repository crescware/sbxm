use std::cell::{Cell, RefCell};

use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::testing::host::AnsweredHost;
use crate::testing::provisioning::World;

/// Sandboxの実体と、独立に壊れたり直ったりするSSH設定・endpoint。
pub struct SshHost {
    pub world: World,
    pub configured: Cell<bool>,
    pub enabled: Cell<bool>,
    pub allowed: Cell<bool>,
    pub endpoint: Cell<bool>,
    pub requires_restart: Cell<bool>,
    pub source: RefCell<String>,
    pub locked: Cell<bool>,
    pub stubborn: Cell<bool>,
    pub refuse: RefCell<Option<&'static str>>,
    pub timeout: RefCell<Option<&'static str>>,
    pub canceled: RefCell<Option<&'static str>>,
    pub probe_stderr: RefCell<Option<&'static str>>,
    pub session_fails: Cell<bool>,
    pub save_fails: Cell<bool>,
    pub calls: RefCell<Vec<CommandSpec>>,
}

impl SshHost {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            configured: Cell::new(true),
            enabled: Cell::new(true),
            allowed: Cell::new(true),
            endpoint: Cell::new(true),
            requires_restart: Cell::new(true),
            source: RefCell::new("override".to_string()),
            locked: Cell::new(false),
            stubborn: Cell::new(false),
            refuse: RefCell::new(None),
            timeout: RefCell::new(None),
            canceled: RefCell::new(None),
            probe_stderr: RefCell::new(None),
            session_fails: Cell::new(false),
            save_fails: Cell::new(false),
            calls: RefCell::new(Vec::new()),
        }
    }

    pub fn invocations(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|spec| format!("{} {}", spec.program, spec.args.join(" ")))
            .collect()
    }

    pub fn mutations(&self) -> Vec<String> {
        self.invocations()
            .into_iter()
            .filter(|call| {
                call == "sbx setup ssh"
                    || call == "sbx settings set feature.ssh true"
                    || call == "sbx daemon restart"
            })
            .collect()
    }
}

impl AnsweredHost for SshHost {
    fn has_command(&self, program: &str) -> bool {
        self.world.has_command(program)
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        let key = format!("{} {}", spec.program, spec.args.join(" "));
        if self
            .canceled
            .borrow()
            .is_some_and(|needle| key.contains(needle))
        {
            return Err(Error::Canceled);
        }
        if self
            .timeout
            .borrow()
            .is_some_and(|needle| key.contains(needle))
        {
            return Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                msg!(
                    "error-external-command-timeout",
                    program = spec.program,
                    seconds = 10
                ),
            ));
        }
        let answer = |code, stdout: &str, stderr: &str| {
            Ok(crate::testing::command::outcome_with_stderr(
                spec, code, stdout, stderr,
            ))
        };
        if self
            .refuse
            .borrow()
            .is_some_and(|needle| key.contains(needle))
        {
            return answer(1, "", "administrator policy rejects this operation");
        }
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        match (spec.program.as_str(), args.as_slice()) {
            ("ssh", ["-G", _]) => answer(0, if self.configured.get() { "proxycommand sbx ssh proxy %n\n" } else { "hostname sandbox.sbx\n" }, ""),
            ("sbx", ["settings", "get", "feature.ssh", "--json"]) => answer(0, &serde_json::json!({
                "key":"feature.ssh", "value":{"enabled":self.enabled.get(),"variant":"","variantPayload":""},
                "source":*self.source.borrow(), "type":"json", "feature_flag":true,
                "requires_restart":self.requires_restart.get(), "locked":self.locked.get(),
            }).to_string(), ""),
            ("sbx", ["settings", "get", "platform.allowExperimentalFeatures", "--json"]) => answer(0,
                &serde_json::json!({"key":"platform.allowExperimentalFeatures","value":self.allowed.get(),"source":"default"}).to_string(), ""),
            ("sbx", ["setup", "ssh"]) => {
                self.configured.set(!self.stubborn.get()); answer(0, "", "")
            }
            ("sbx", ["settings", "set", "feature.ssh", "true"]) => {
                self.enabled.set(!self.stubborn.get());
                if !self.requires_restart.get() { self.endpoint.set(true); }
                answer(0, "Updated setting feature.ssh", "")
            }
            ("sbx", ["daemon", "restart"]) => {
                self.endpoint.set(!self.stubborn.get()); answer(0, "", "")
            }
            ("ssh", ["-n", ..]) => {
                if let Some(stderr) = *self.probe_stderr.borrow() {
                    answer(255, "", stderr)
                } else if self.enabled.get() && self.allowed.get() && self.endpoint.get() {
                    answer(0, "", "")
                } else {
                    answer(255, "", "error: the sandboxd SSH endpoint is disabled or unavailable; enable it only if administrator policy permits")
                }
            }
            ("ssh", ["-t", ..]) if self.session_fails.get() => answer(255, "", "connection lost"),
            ("git", _) if self.save_fails.get() && args.contains(&"fetch") => answer(128, "", "SSH endpoint unavailable"),
            ("sbx", _) if self.save_fails.get() && args.contains(&crate::support::host_sync::PLACE_SAVE_REFS) => answer(0, "ready\n", ""),
            _ => self.world.answer(spec),
        }
    }
}

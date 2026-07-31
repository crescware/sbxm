use std::collections::HashMap;

use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

pub struct FakeHost {
    present: Vec<String>,
    responses: HashMap<String, std::result::Result<(String, String, i32), ErrorId>>,
}

impl FakeHost {
    pub fn new() -> FakeHost {
        FakeHost {
            present: Vec::new(),
            responses: HashMap::new(),
        }
    }

    pub fn with_commands(mut self, programs: &[&str]) -> FakeHost {
        self.present = programs.iter().map(|value| (*value).to_string()).collect();
        self
    }

    pub fn responding(mut self, key: &str, stdout: &str) -> FakeHost {
        self.responses
            .insert(key.to_string(), Ok((stdout.to_string(), String::new(), 0)));
        self
    }

    pub fn failing(mut self, key: &str, stderr: &str, code: i32) -> FakeHost {
        self.responses.insert(
            key.to_string(),
            Ok((String::new(), stderr.to_string(), code)),
        );
        self
    }

    pub fn timing_out(mut self, key: &str) -> FakeHost {
        self.responses
            .insert(key.to_string(), Err(ErrorId::ExternalCommandTimeout));
        self
    }

    pub fn macos() -> FakeHost {
        FakeHost::new()
            .with_commands(&["git", "ssh", "docker", "sbx"])
            .responding("sw_vers -productVersion", "14.5\n")
            .responding("uname -m", "arm64\n")
            .responding("docker version --format {{.Server.Version}}", "27.0.3\n")
            .responding("sbx version", "sbx version 0.37.0\n")
            .responding("git config --global --get-all user.name", "Example User\n")
            .responding(
                "git config --global --get-all user.email",
                "user@example.com\n",
            )
    }
}

impl HostEnvironment for FakeHost {
    fn command_exists(&self, program: &str) -> bool {
        self.present.iter().any(|value| value == program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let key = if spec.args.is_empty() {
            spec.program.clone()
        } else {
            format!("{} {}", spec.program, spec.args.join(" "))
        };
        match self.responses.get(&key) {
            Some(Ok((stdout, stderr, code))) => Ok(crate::testing::command::outcome_with_stderr(
                spec, *code, stdout, stderr,
            )),
            Some(Err(ErrorId::ExternalCommandTimeout)) => Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                msg!(
                    "error-external-command-timeout",
                    program = spec.program,
                    seconds = 10
                ),
            )),
            Some(Err(id)) => Err(Error::new(
                *id,
                msg!("error-external-command-not-found", program = spec.program),
            )),
            None => Err(Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = spec.program),
            )),
        }
    }
}
